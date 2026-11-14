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
fn test_dynamic_wages_evolution_and_bounds() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).expect("Failed to load defs");
    let mut world = DataLoader::load_world(&data_dir.join("scenarios").join("2020_default"), defs)
        .expect("Failed to load scenario");

    let initial_wages: Vec<f64> = world
        .facilities
        .iter()
        .flat_map(|f| f.lines.iter().map(|l| l.target_wage))
        .collect();

    assert!(!initial_wages.is_empty());

    for _ in 0..1440 {
        world.tick();
    }

    let mid_wages: Vec<f64> = world
        .facilities
        .iter()
        .flat_map(|f| f.lines.iter().map(|l| l.target_wage))
        .collect();

    let mut changed_lines = 0;
    for (init, current) in initial_wages.iter().zip(mid_wages.iter()) {
        if (init - current).abs() > 1e-4 {
            changed_lines += 1;
        }
    }

    assert!(
        changed_lines > 0,
        "Expected at least some line target wages to change dynamically after 2 months"
    );

    for facility in &world.facilities {
        let country = world
            .countries
            .iter()
            .find(|c| c.tag == facility.country_tag)
            .expect("Facility country must exist");
        let ex_rate = country.economy.exchange_rate.max(0.001);
        let base_wage = ex_rate.max(1.0);
        let min_bound = base_wage * 0.4;
        let max_bound = base_wage * 50.0;

        for line in &facility.lines {
            assert!(
                line.target_wage >= min_bound - 1e-5 && line.target_wage <= max_bound + 1e-5,
                "Line wage {} out of bounds [{}, {}] for country {}",
                line.target_wage,
                min_bound,
                max_bound,
                facility.country_tag.as_str()
            );
        }
    }

    for region in &world.regions {
        if let Some(owner) = region.owner {
            let country = world
                .countries
                .iter()
                .find(|c| c.tag == owner)
                .expect("Region country must exist");
            let ex_rate = country.economy.exchange_rate.max(0.001);
            let base_wage = ex_rate.max(1.0) * 30.0;
            let min_bound = base_wage * 0.4;
            let max_bound = base_wage * 50.0;

            for structure in &region.structures {
                assert!(
                    structure.wage_per_worker >= min_bound - 1e-5
                        && structure.wage_per_worker <= max_bound + 1e-5,
                    "Structure wage {} out of bounds [{}, {}] for country {}",
                    structure.wage_per_worker,
                    min_bound,
                    max_bound,
                    owner.as_str()
                );
            }
        }
    }
}

#[test]
fn test_dynamic_wages_determinism() {
    let data_dir = find_data_dir();
    let defs1 = DataLoader::load_game_defs(&data_dir.join("common")).expect("Failed to load defs");
    let mut world1 =
        DataLoader::load_world(&data_dir.join("scenarios").join("2020_default"), defs1)
            .expect("Failed to load scenario");

    let defs2 = DataLoader::load_game_defs(&data_dir.join("common")).expect("Failed to load defs");
    let mut world2 =
        DataLoader::load_world(&data_dir.join("scenarios").join("2020_default"), defs2)
            .expect("Failed to load scenario");

    for _ in 0..2160 {
        world1.tick();
        world2.tick();
    }

    for (f1, f2) in world1.facilities.iter().zip(world2.facilities.iter()) {
        for (l1, l2) in f1.lines.iter().zip(f2.lines.iter()) {
            assert_eq!(
                l1.target_wage.to_bits(),
                l2.target_wage.to_bits(),
                "Target wage must be strictly bit-identical across deterministic runs"
            );
        }
    }
}
