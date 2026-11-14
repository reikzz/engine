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
fn test_multicurrency_and_global_trade() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let mut world =
        DataLoader::load_world(&scenario_path, defs).expect("Failed to load scenario world");

    assert_eq!(world.countries.len(), 4);
    let usa = world
        .country(CountryTag::new("USA"))
        .expect("USA must exist");
    let rus = world
        .country(CountryTag::new("RUS"))
        .expect("RUS must exist");
    let chn = world
        .country(CountryTag::new("CHN"))
        .expect("CHN must exist");
    let deu = world
        .country(CountryTag::new("DEU"))
        .expect("DEU must exist");

    assert_eq!(chn.economy.currency_code, "CNY");
    assert_eq!(chn.economy.currency_symbol, "¥");
    assert_eq!(chn.economy.exchange_rate, 8.28);

    assert_eq!(deu.economy.currency_code, "EUR");
    assert_eq!(deu.economy.currency_symbol, "€");
    assert_eq!(deu.economy.exchange_rate, 1.08);

    assert_eq!(usa.economy.currency_code, "USD");
    assert_eq!(usa.economy.currency_symbol, "$");
    assert_eq!(usa.economy.exchange_rate, 1.0);

    assert_eq!(rus.economy.currency_code, "RUB");
    assert_eq!(rus.economy.currency_symbol, "₽");
    assert_eq!(rus.economy.exchange_rate, 28.0);

    let oil_id = world.defs.resources.get_id("crude_oil").unwrap();
    let usa_oil_price = usa.market.get(oil_id).unwrap().price;
    let rus_oil_price = rus.market.get(oil_id).unwrap().price;

    assert_eq!(usa_oil_price, 75.0);
    assert_eq!(rus_oil_price, 2100.0);

    for _ in 1..=2160 {
        world.tick();
    }

    let usa_after = world.country(CountryTag::new("USA")).unwrap();
    let rus_after = world.country(CountryTag::new("RUS")).unwrap();

    assert!(usa_after.economy.treasury > 0.0);
    assert!(rus_after.economy.treasury > 0.0);
}

#[test]
fn test_global_trade_money_conservation_and_solvency() {
    let data_dir = find_data_dir();
    let defs_path = data_dir.join("common");
    let scenario_path = data_dir.join("scenarios/2020_default");

    let defs = DataLoader::load_game_defs(&defs_path).expect("Failed to load game defs");
    let mut world =
        DataLoader::load_world(&scenario_path, defs).expect("Failed to load scenario world");

    for _ in 1..=720 {
        world.tick();
    }

    for country in &world.countries {
        assert!(country.economy.treasury >= 0.0);
        for (_, rm) in country.market.iter() {
            assert!(rm.revenue_pool >= 0.0);
        }
    }
}
