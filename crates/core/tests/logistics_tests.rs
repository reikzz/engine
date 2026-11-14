use engine_core::economy::SectorKind;
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
fn test_logistics_sector_weight_differences() {
    let heavy = SectorKind::HeavyIndustry.infrastructure_weight();
    let extraction = SectorKind::Extraction.infrastructure_weight();
    let it = SectorKind::InformationTechnology.infrastructure_weight();
    let finance = SectorKind::Financial.infrastructure_weight();

    assert!(heavy > it);
    assert!(extraction > finance);
    assert_eq!(heavy, 1.2);
    assert_eq!(it, 0.05);
}

#[test]
fn test_logistics_system_initialization_and_metrics() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let mut world = DataLoader::load_world(&scenario_path, defs).expect("Failed to load world");

    for _ in 0..24 {
        world.tick();
    }

    for region in &world.regions {
        assert!(
            region.infrastructure_capacity >= 10.0,
            "Region {} capacity {} must be >= 10.0",
            region.id.0,
            region.infrastructure_capacity
        );
        assert!(
            region.infrastructure_usage >= 0.0,
            "Region {} usage {} must be >= 0.0",
            region.id.0,
            region.infrastructure_usage
        );
        assert!(
            region.market_access >= 0.05 && region.market_access <= 1.0,
            "Region {} market access {} must be in [0.05, 1.0]",
            region.id.0,
            region.market_access
        );
    }
}

#[test]
fn test_market_access_drop_on_overload() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let mut world = DataLoader::load_world(&scenario_path, defs).expect("Failed to load world");

    if let Some(target_region) = world.regions.first_mut() {
        target_region.infrastructure_level = 0.0;
        target_region.structures.clear();
    }

    if let Some(first_fac) = world.facilities.first_mut() {
        first_fac.sector_kind = SectorKind::HeavyIndustry;
        for line in &mut first_fac.lines {
            line.installed_capacity = 50000;
        }
    }

    for _ in 0..24 {
        world.tick();
    }

    let first_region = &world.regions[0];
    assert!(
        first_region.infrastructure_usage > 1000.0,
        "Usage should be high due to huge factory capacity"
    );
    assert!(
        first_region.market_access < 0.5,
        "Market access must degrade below 0.5 under extreme load: got {}",
        first_region.market_access
    );
}

#[test]
fn test_price_blending_on_low_market_access() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let mut world = DataLoader::load_world(&scenario_path, defs).expect("Failed to load world");

    let region = &mut world.regions[0];
    region.reset_local_market_tracking(world.defs.resources.len());
    region.market_access = 0.40;

    let base_prices = vec![100.0];
    let national_prices = vec![100.0];

    region.local_supplied[0] = 1000.0;
    region.local_demanded[0] = 10.0;
    region.update_local_market(&base_prices, &national_prices);

    let national_price = 100.0;
    let effective_price = region.effective_price(0, national_price);

    assert!(
        region.local_prices[0] < national_price,
        "Local oversupply must drive local price down: got {}",
        region.local_prices[0]
    );
    assert!(
        effective_price < national_price,
        "Effective price {} must be lower than national price {} due to blending",
        effective_price,
        national_price
    );

    region.market_access = 1.0;
    let full_access_price = region.effective_price(0, national_price);
    assert_eq!(full_access_price, national_price);
}
