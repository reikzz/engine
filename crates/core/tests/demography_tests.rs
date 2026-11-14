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
fn test_demography_growth_and_migration() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let mut world =
        DataLoader::load_world(&scenario_path, defs).expect("Failed to load scenario world");

    let initial_pop: u64 = world
        .regions
        .iter()
        .flat_map(|r| &r.pops)
        .map(|p| p.count())
        .sum();
    assert!(initial_pop >= 182300);

    for _ in 1..=4320 {
        world.tick();
    }

    let pop_after_6_months: u64 = world
        .regions
        .iter()
        .flat_map(|r| &r.pops)
        .map(|p| p.count())
        .sum();

    assert!(
        pop_after_6_months > initial_pop,
        "Population should grow with adequate food supply (before: {}, after: {})",
        initial_pop,
        pop_after_6_months
    );

    for r in &world.regions {
        let r_pop: u64 = r.pops.iter().map(|p| p.count()).sum();
        let r_savings: f64 = r.pops.iter().map(|p| p.total_savings()).sum();
        assert!(r_pop > 0, "Region pop must be positive");
        assert!(r_savings >= 0.0, "Region savings must be non-negative");
    }
}
