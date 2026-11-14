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
fn test_central_bank_rate_action() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let tag = CountryTag::new("USA");
    let rate = world.set_central_bank_rate(tag, 0.075).unwrap();
    assert_eq!(rate, 0.075);
    assert_eq!(world.country(tag).unwrap().economy.central_bank_rate, 0.075);

    let clamped_high = world.set_central_bank_rate(tag, 0.50).unwrap();
    assert_eq!(clamped_high, 0.30);
    assert_eq!(world.country(tag).unwrap().economy.central_bank_rate, 0.30);

    let clamped_low = world.set_central_bank_rate(tag, -0.05).unwrap();
    assert_eq!(clamped_low, 0.0);
    assert_eq!(world.country(tag).unwrap().economy.central_bank_rate, 0.0);
}

#[test]
fn test_cpi_and_inflation_evolution() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let tag = CountryTag::new("USA");
    let initial_cpi = world.country(tag).unwrap().economy.cpi;
    assert_eq!(initial_cpi, 100.0);

    for _ in 0..35 {
        world.tick_day();
    }

    let country = world.country(tag).unwrap();
    assert!(country.economy.cpi > 10.0 && country.economy.cpi < 500.0);
    assert!(country.economy.cpi_history_len >= 1);
    assert!(country.economy.inflation_rate >= -0.10 && country.economy.inflation_rate <= 0.50);
}

#[test]
fn test_national_debt_service() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let tag = CountryTag::new("USA");
    world.set_central_bank_rate(tag, 0.10).unwrap();
    world.take_national_loan(tag, 100_000.0).unwrap();

    let expected_daily_service = 100_000.0 * 0.10 / 365.0;
    let expenses_before = world.country(tag).unwrap().economy.last_period_expenses;

    world.tick_day();

    let expenses_after = world.country(tag).unwrap().economy.last_period_expenses;
    let expenses_delta = expenses_after - expenses_before;
    assert!(
        expenses_delta >= expected_daily_service * 0.99,
        "Expenses delta {} should be at least debt service {}",
        expenses_delta,
        expected_daily_service
    );
}
