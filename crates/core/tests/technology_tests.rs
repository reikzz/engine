use engine_core::CountryTag;
use engine_core::loader::DataLoader;
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
fn test_technology_definitions_and_prerequisites_loaded() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();

    assert!(defs.technologies.count() >= 10);

    let semi_tech_id = defs
        .technologies
        .get_id("tech_advanced_semiconductors")
        .expect("tech_advanced_semiconductors must exist");
    let semi_def = defs.technologies.get(semi_tech_id).unwrap();

    let micro_tech_id = defs
        .technologies
        .get_id("tech_microelectronics")
        .expect("tech_microelectronics must exist");

    assert!(semi_def.prerequisites.contains(&micro_tech_id));

    let semi_recipe_id = defs
        .recipes
        .get_id("recipe_semiconductors")
        .expect("recipe_semiconductors must exist");
    let semi_recipe = defs.recipes.get(semi_recipe_id).unwrap();

    assert_eq!(semi_recipe.required_tech, Some(semi_tech_id));
}

#[test]
fn test_country_starting_technologies() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let semi_tech_id = world
        .defs
        .technologies
        .get_id("tech_advanced_semiconductors")
        .unwrap();
    let micro_tech_id = world
        .defs
        .technologies
        .get_id("tech_microelectronics")
        .unwrap();

    let usa = world.country(CountryTag::new("USA")).unwrap();
    let rus = world.country(CountryTag::new("RUS")).unwrap();
    let deu = world.country(CountryTag::new("DEU")).unwrap();

    assert!(usa.has_technology(semi_tech_id));
    assert!(usa.has_technology(micro_tech_id));

    assert!(!rus.has_technology(semi_tech_id));
    assert!(!rus.has_technology(micro_tech_id));
    assert_eq!(rus.active_research, Some(micro_tech_id));

    assert!(!deu.has_technology(semi_tech_id));
    assert!(deu.has_technology(micro_tech_id));
    assert_eq!(deu.active_research, Some(semi_tech_id));
}

#[test]
fn test_technology_blocks_high_tech_construction() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let semi_recipe_id = world.defs.recipes.get_id("recipe_semiconductors").unwrap();

    let rus_tag = CountryTag::new("RUS");
    let rus_comp = world
        .companies
        .iter()
        .find(|c| c.country_tag == rus_tag)
        .map(|c| c.id)
        .unwrap();
    let rus_region = world
        .regions
        .iter()
        .find(|r| r.owner == Some(rus_tag))
        .map(|r| r.id)
        .unwrap();

    let rus_result =
        world.commission_facility_construction(rus_tag, rus_comp, semi_recipe_id, rus_region, None);
    assert!(rus_result.is_err());
    assert_eq!(
        rus_result.unwrap_err(),
        "Required technology is not unlocked".to_string()
    );

    let usa_tag = CountryTag::new("USA");
    let usa_comp = world
        .companies
        .iter()
        .find(|c| c.country_tag == usa_tag)
        .map(|c| c.id)
        .unwrap();
    let usa_region = world
        .regions
        .iter()
        .find(|r| r.owner == Some(usa_tag))
        .map(|r| r.id)
        .unwrap();

    let usa_result =
        world.commission_facility_construction(usa_tag, usa_comp, semi_recipe_id, usa_region, None);
    assert!(usa_result.is_ok());
}

#[test]
fn test_research_actions_and_progression() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let rus_tag = CountryTag::new("RUS");
    let semi_tech_id = world
        .defs
        .technologies
        .get_id("tech_advanced_semiconductors")
        .unwrap();
    let micro_tech_id = world
        .defs
        .technologies
        .get_id("tech_microelectronics")
        .unwrap();

    let invalid_set = world.set_active_research(rus_tag, semi_tech_id);
    assert!(invalid_set.is_err());
    assert_eq!(invalid_set.unwrap_err(), "Prerequisites not satisfied");

    let valid_set = world.set_active_research(rus_tag, micro_tech_id);
    assert!(valid_set.is_ok());

    let initial_progress = world.country(rus_tag).unwrap().research_progress;
    assert_eq!(initial_progress, 0.0);

    for _ in 0..35 {
        world.tick_day();
    }

    let country = world.country(rus_tag).unwrap();
    assert!(country.research_progress > 0.0 || country.has_technology(micro_tech_id));
    assert!(country.last_period_science_output > 0.0);
}
