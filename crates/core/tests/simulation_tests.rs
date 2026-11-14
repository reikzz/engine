use engine_core::loader::DataLoader;
use engine_core::world::time::{GameDate, TimeTick};
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
fn test_game_date_progression() {
    let mut date = GameDate::default();
    assert_eq!(date.year(), 2020);
    assert_eq!(date.month(), 1);
    assert_eq!(date.day(), 1);
    assert_eq!(date.hour(), 0);

    for h in 1..24 {
        let tick = date.advance();
        assert_eq!(tick, TimeTick::Hour);
        assert!(!tick.is_new_day());
        assert_eq!(date.hour(), h);
        assert_eq!(date.day(), 1);
    }

    let day_tick = date.advance();
    assert_eq!(day_tick, TimeTick::Day);
    assert!(day_tick.is_new_day());
    assert!(!day_tick.is_new_month());
    assert_eq!(date.hour(), 0);
    assert_eq!(date.day(), 2);

    for _ in 0..695 {
        date.advance();
    }
    let month_tick = date.advance();
    assert!(month_tick.is_new_month());
    assert_eq!(date.month(), 2);
    assert_eq!(date.day(), 1);
    assert_eq!(date.hour(), 0);
}

#[test]
fn test_simulation_first_day_execution() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(data_dir.join("scenarios/2020_default"), defs).unwrap();

    let oil_id = world.defs.resources.get_id("crude_oil").unwrap();

    let oil_facility_idx = world
        .facilities
        .iter()
        .position(|f| {
            f.lines
                .iter()
                .any(|l| l.recipe_id == world.defs.recipes.get_id("recipe_oil_drilling").unwrap())
        })
        .expect("Oil facility must exist");

    let initial_oil = world.facilities[oil_facility_idx].stockpile.get(oil_id);

    for _ in 1..=24 {
        world.tick();
    }

    let after_oil = world.facilities[oil_facility_idx].stockpile.get(oil_id);
    assert!(
        after_oil > initial_oil,
        "Facility should produce crude oil on the first day, before: {initial_oil}, after: {after_oil}"
    );

    let oil_line = world.facilities[oil_facility_idx]
        .lines
        .iter()
        .find(|l| l.recipe_id == world.defs.recipes.get_id("recipe_oil_drilling").unwrap())
        .expect("Oil line must exist");
    assert!(oil_line.last_day_output > 0.0);
}

#[test]
fn test_simulation_determinism() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs1 = DataLoader::load_game_defs(&defs_path).unwrap();
    let mut world1 = DataLoader::load_world(&scenario_path, defs1).unwrap();

    let defs2 = DataLoader::load_game_defs(&defs_path).unwrap();
    let mut world2 = DataLoader::load_world(&scenario_path, defs2).unwrap();

    for (r1, r2) in world1.regions.iter().zip(&world2.regions) {
        assert_eq!(r1.pops.len(), r2.pops.len());
        for (p1, p2) in r1.pops.iter().zip(&r2.pops) {
            assert_eq!(p1.pop_type_id, p2.pop_type_id);
            assert_eq!(p1.count(), p2.count());
        }
    }

    let oil_id = world1.defs.resources.get_id("crude_oil").unwrap();

    for _ in 0..1000 {
        world1.tick();
        world2.tick();
    }

    assert_eq!(
        world1.facilities[0].stockpile.get(oil_id),
        world2.facilities[0].stockpile.get(oil_id)
    );
    assert_eq!(world1.companies[0].balance, world2.companies[0].balance);
    assert_eq!(
        world1.countries[0].economy.treasury,
        world2.countries[0].economy.treasury
    );
    assert_eq!(
        world1.countries[1].economy.treasury,
        world2.countries[1].economy.treasury
    );

    for _ in 1000..8640 {
        world1.tick();
        world2.tick();
    }

    assert_eq!(
        world1.countries[0].economy.treasury,
        world2.countries[0].economy.treasury
    );
    assert_eq!(
        world1.countries[1].economy.treasury,
        world2.countries[1].economy.treasury
    );
}

#[test]
fn test_nominal_gdp_calculation_and_evolution() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).unwrap();
    let mut world = DataLoader::load_world(&scenario_path, defs).unwrap();

    for country in &world.countries {
        assert!(
            country.economy.nominal_gdp > 0.0,
            "Initial nominal GDP for {} must be positive, got {}",
            country.tag,
            country.economy.nominal_gdp
        );
    }

    for _ in 0..720 {
        world.tick();
    }

    for country in &world.countries {
        assert!(
            country.economy.nominal_gdp > 0.0,
            "Monthly updated nominal GDP for {} must be positive, got {}",
            country.tag,
            country.economy.nominal_gdp
        );
    }
}
