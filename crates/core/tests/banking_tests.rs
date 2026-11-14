use engine_core::economy::{Ownership, SectorKind};
use engine_core::loader::DataLoader;
use engine_core::world::BailoutPolicy;
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
fn test_banking_system_initialization_and_operations() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let mut world = DataLoader::load_world(&scenario_path, defs).expect("Failed to load world");

    let usa_state_bank = world.companies.iter().find(|c| {
        c.country_tag.as_str() == "USA"
            && c.sector_kind == SectorKind::Financial
            && c.ownership == Ownership::State
    });
    assert!(usa_state_bank.is_some(), "USA State bank must be present");

    let rus_state_bank = world.companies.iter().find(|c| {
        c.country_tag.as_str() == "RUS"
            && c.sector_kind == SectorKind::Financial
            && c.ownership == Ownership::State
    });
    assert!(rus_state_bank.is_some(), "RUS State bank must be present");

    let rus_priv_bank = world.companies.iter().find(|c| {
        c.country_tag.as_str() == "RUS"
            && c.sector_kind == SectorKind::Financial
            && matches!(c.ownership, Ownership::Private { .. })
    });
    assert!(rus_priv_bank.is_some(), "RUS Private bank must be present");

    for _ in 0..1440 {
        world.tick();
    }

    let final_pop_savings: f64 = world
        .regions
        .iter()
        .flat_map(|r| &r.pops)
        .map(|p| p.total_savings())
        .sum();
    assert!(
        final_pop_savings > 0.0,
        "Pops must accumulate savings after wages and deposit interest"
    );

    assert_eq!(world.countries.len(), 4);
}

#[test]
fn test_bailout_policy_controls_treasury_subsidies() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let mut world = DataLoader::load_world(&scenario_path, defs).expect("Failed to load world");

    for country in &mut world.countries {
        country.economy.bailout_policy = BailoutPolicy::Disabled;
        country.economy.max_subsidy_budget_ratio = 0.0;
        country.structures.clear();
    }
    for region in &mut world.regions {
        region.structures.clear();
    }

    let initial_treasuries: Vec<(String, f64)> = world
        .countries
        .iter()
        .map(|c| (c.tag.as_str().to_string(), c.economy.treasury))
        .collect();

    for _ in 0..720 {
        world.tick();
    }

    for (tag, initial_tr) in initial_treasuries {
        let current_tr = world
            .countries
            .iter()
            .find(|c| c.tag.as_str() == tag)
            .map(|c| c.economy.treasury)
            .unwrap_or(0.0);

        assert!(
            current_tr >= initial_tr,
            "With disabled bailout policy treasury should not spend funds on subsidies"
        );
    }
}
