use engine_core::CountryTag;
use engine_core::economy::{DigitalPlatformKind, SectorKind};
use engine_core::loader::DataLoader;
use engine_core::world::map::RegionModifier;
use std::path::PathBuf;

fn find_data_dir() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap();
    loop {
        let candidate = dir.join("data");
        if candidate.exists() {
            return candidate;
        }
        if !dir.pop() {
            panic!("Could not find data directory");
        }
    }
}

#[test]
fn test_digital_platforms_initialization_and_sync() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    assert!(!world.digital_platforms.is_empty());

    let initial_count = world.digital_platforms.len();

    for _ in 0..32 {
        world.tick_day();
    }

    assert!(world.digital_platforms.len() >= initial_count);

    let has_consumer = world
        .digital_platforms
        .iter()
        .any(|p| p.kind == DigitalPlatformKind::ConsumerServices);
    let has_cloud = world
        .digital_platforms
        .iter()
        .any(|p| p.kind == DigitalPlatformKind::EnterpriseCloud);
    let has_software = world
        .digital_platforms
        .iter()
        .any(|p| p.kind == DigitalPlatformKind::BusinessSoftware);

    assert!(has_consumer);
    assert!(has_cloud);
    assert!(has_software);

    for p in &world.digital_platforms {
        assert!(p.max_subscribers > 0);
        assert!(p.monthly_fee > 0.0);
        assert!(p.quality >= 0.5);
    }
}

#[test]
fn test_subscribers_distribution_and_recurring_revenue() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let tech_companies: Vec<_> = world
        .companies
        .iter()
        .filter(|c| c.sector_kind == SectorKind::InformationTechnology)
        .map(|c| (c.id, c.balance))
        .collect();

    assert!(!tech_companies.is_empty());

    for _ in 0..35 {
        world.tick_day();
    }

    let mut total_subscribers = 0u64;
    let mut total_mrr = 0.0;

    for p in &world.digital_platforms {
        total_subscribers += p.active_subscribers;
        total_mrr += p.monthly_revenue;
    }

    assert!(total_subscribers > 0);
    assert!(total_mrr > 0.0);

    let active_it_companies_with_revenue = world
        .digital_platforms
        .iter()
        .filter(|p| p.monthly_revenue > 0.0)
        .count();
    assert!(active_it_companies_with_revenue > 0);
}

#[test]
fn test_digital_modifiers_and_cross_border_access() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    for _ in 0..65 {
        world.tick_day();
    }

    let regions_with_productivity = world
        .regions
        .iter()
        .filter(|r| r.modifiers.get(RegionModifier::LaborProductivity) > 0.0)
        .count();

    assert!(regions_with_productivity > 0);

    let us_tag = CountryTag::new("USA");
    let rus_tag = CountryTag::new("RUS");

    let us_platforms: Vec<_> = world
        .digital_platforms
        .iter()
        .filter(|p| p.country_tag == us_tag)
        .collect();
    let rus_platforms: Vec<_> = world
        .digital_platforms
        .iter()
        .filter(|p| p.country_tag == rus_tag)
        .collect();

    assert!(!us_platforms.is_empty());
    assert!(!rus_platforms.is_empty());
}
