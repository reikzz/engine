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
fn test_load_game_defs_from_data_common() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");

    assert!(defs.resources.count() >= 2);
    let oil_id = defs
        .resources
        .get_id("crude_oil")
        .expect("crude_oil resource must exist");
    let cg_id = defs
        .resources
        .get_id("fuel")
        .expect("fuel resource must exist");

    let oil_def = defs
        .resources
        .get(oil_id)
        .expect("crude_oil def must exist");
    assert_eq!(oil_def.key, "crude_oil");
    assert_eq!(oil_def.base_price, 75.0);

    let cg_def = defs.resources.get(cg_id).expect("fuel def must exist");
    assert_eq!(cg_def.key, "fuel");
    assert_eq!(cg_def.base_price, 85.0);

    assert!(defs.pop_types.count() >= 5);
    assert!(defs.pop_types.get_id("workers").is_some());
    assert!(defs.pop_types.get_id("professionals").is_some());
    assert!(defs.pop_types.get_id("engineers").is_some());

    assert!(defs.recipes.get_id("recipe_oil_drilling").is_some());
    assert!(defs.recipes.get_id("recipe_oil_refining").is_some());
}

#[test]
fn test_load_world_from_scenario() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let world =
        DataLoader::load_world(&scenario_path, defs).expect("Failed to load scenario world");

    assert!(!world.countries.is_empty());
    let usa = world
        .country(CountryTag::new("USA"))
        .expect("USA must exist");
    assert_eq!(usa.tag, CountryTag::new("USA"));
    assert_eq!(usa.economy.corporate_tax, 0.20);
    assert_eq!(usa.economy.income_tax, 0.15);

    assert!(!world.regions.is_empty());
    assert!(!world.provinces.is_empty());

    assert!(world.companies.len() >= 2);
    assert!(world.facilities.len() >= 2);

    assert_eq!(world.date.year(), 2000);
    assert_eq!(world.date.month(), 1);
    assert_eq!(world.date.day(), 1);
    assert_eq!(world.date.hour(), 0);
}

#[test]
fn test_load_non_existent_scenario_fails() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");

    let result = DataLoader::load_world("data/non_existent_scenario_dir_12345", defs);
    assert!(result.is_err());
}
