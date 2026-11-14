use engine_core::economy::finance::BudgetSource;
use engine_core::economy::{RealEstateKind, SectorKind};
use engine_core::loader::DataLoader;
use engine_core::world::CountryTag;
use engine_core::world::map::RegionId;
use std::path::{Path, PathBuf};

fn find_data_dir() -> PathBuf {
    let candidates = [
        Path::new("data"),
        Path::new("../../data"),
        Path::new("../data"),
    ];

    for candidate in candidates {
        if candidate.join("common").exists() {
            return candidate.to_path_buf();
        }
    }
    panic!("Could not find 'data' directory in candidate paths");
}

#[test]
fn test_real_estate_initialization_from_scenario() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let world = DataLoader::load_world(&scenario_path, defs).expect("Failed to load world");

    assert!(
        !world.real_estate.is_empty(),
        "World must contain real estate properties"
    );

    let office = world
        .real_estate
        .iter()
        .find(|re| re.kind == RealEstateKind::CommercialOffice);
    assert!(
        office.is_some(),
        "CommercialOffice property must be present"
    );
    let office = office.unwrap();
    assert!(
        office.total_units > 0,
        "CommercialOffice must have positive units"
    );
    assert!(
        office.base_rent_per_unit > 0.0,
        "CommercialOffice must have positive rent"
    );

    let housing = world
        .real_estate
        .iter()
        .find(|re| re.kind == RealEstateKind::ResidentialHousing);
    assert!(
        housing.is_some(),
        "ResidentialHousing property must be present"
    );
    let housing = housing.unwrap();
    assert!(
        housing.total_units > 0,
        "ResidentialHousing must have positive units"
    );
}

#[test]
fn test_it_sector_defs_and_recipes() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");

    assert!(defs.resources.get_id("software").is_some());
    assert!(defs.resources.get_id("cloud_services").is_some());
    assert!(defs.resources.get_id("digital_services").is_some());

    assert!(defs.recipes.get_id("recipe_software_development").is_some());
    assert!(defs.recipes.get_id("recipe_cloud_hosting").is_some());
    assert!(defs.recipes.get_id("recipe_digital_platforms").is_some());

    let it_recipe_id = defs
        .recipes
        .get_id("recipe_software_development")
        .expect("Recipe id must exist");
    let it_recipe = defs.recipes.get(it_recipe_id).expect("Recipe must exist");
    assert_eq!(it_recipe.sector, SectorKind::InformationTechnology);
}

#[test]
fn test_commission_and_build_real_estate() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let mut world = DataLoader::load_world(&scenario_path, defs).expect("Failed to load world");

    let tag = CountryTag::new("USA");
    let region_id = RegionId(0);
    let owner_comp = world
        .companies
        .iter()
        .find(|c| c.country_tag == tag)
        .map(|c| c.id);

    let initial_count = world.real_estate.len();

    let res = world.commission_real_estate_construction(
        tag,
        region_id,
        RealEstateKind::CommercialOffice,
        5000,
        150.0,
        owner_comp,
        None,
        BudgetSource::Federal(tag),
    );

    assert!(
        res.is_ok(),
        "Commissioning real estate should succeed: {:?}",
        res
    );

    let country = world.country_mut(tag).unwrap();
    assert_eq!(country.construction_queue.len(), 1);
    let project = &mut country.construction_queue[0];
    project.progress = project.total_days;

    let tick = world.tick_day();
    assert!(tick.is_new_day());

    assert_eq!(
        world.real_estate.len(),
        initial_count + 1,
        "New real estate property should be spawned"
    );

    let new_prop = world.real_estate.last().unwrap();
    assert_eq!(new_prop.kind, RealEstateKind::CommercialOffice);
    assert_eq!(new_prop.total_units, 5000);
    assert_eq!(new_prop.base_rent_per_unit, 150.0);
    assert_eq!(new_prop.owner_company_id, owner_comp);
}

#[test]
fn test_real_estate_and_it_simulation_cycle() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let mut world = DataLoader::load_world(&scenario_path, defs).expect("Failed to load world");

    let office_owner_id = world
        .real_estate
        .iter()
        .find(|re| re.kind == RealEstateKind::CommercialOffice)
        .and_then(|re| re.owner_company_id)
        .expect("Office owner company must exist");

    let initial_balance = world
        .companies
        .iter()
        .find(|c| c.id == office_owner_id)
        .map(|c| c.balance)
        .unwrap();

    for _ in 0..35 {
        world.tick_day();
    }

    let updated_prop = world
        .real_estate
        .iter()
        .find(|re| re.kind == RealEstateKind::CommercialOffice)
        .unwrap();

    assert!(
        updated_prop.occupied_units > 0,
        "Office units should be occupied by white-collar workforce"
    );
    assert!(
        updated_prop.period_revenue > 0.0,
        "Office should generate rental revenue"
    );

    let new_balance = world
        .companies
        .iter()
        .find(|c| c.id == office_owner_id)
        .map(|c| c.balance)
        .unwrap();

    assert!(
        new_balance > initial_balance,
        "Real estate company balance should increase from net rental revenue: before={initial_balance}, after={new_balance}"
    );
}
