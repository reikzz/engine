use engine_core::economy::ResourceId;
use engine_core::economy::market::{MarketConfig, NationalMarket};

#[test]
fn test_national_market_initialization() {
    let base_prices = [100.0, 50.0, 25.0];
    let market = NationalMarket::new(&base_prices);

    assert_eq!(market.price(ResourceId::from(0)), 100.0);
    assert_eq!(market.price(ResourceId::from(1)), 50.0);
    assert_eq!(market.price(ResourceId::from(2)), 25.0);
    assert_eq!(market.price(ResourceId::from(3)), 0.0);
}

#[test]
fn test_market_price_rises_on_shortage() {
    let base_prices = [100.0];
    let mut market = NationalMarket::new(&base_prices);
    let res = ResourceId::from(0);

    market.record_demand(res, 200.0);
    market.record_supply(res, 50.0);
    market.update_prices();

    let entry = market.get(res).expect("resource market entry should exist");
    assert!(
        entry.price > 100.0,
        "Price should increase when demand exceeds supply, got {}",
        entry.price
    );
    assert_eq!(entry.sell_fulfillment, 1.0);
    assert!((entry.buy_fulfillment - 0.25).abs() < 1e-6);
    assert_eq!(entry.last_demanded, 200.0);
    assert_eq!(entry.last_supplied, 50.0);
}

#[test]
fn test_market_price_drops_on_oversupply() {
    let base_prices = [100.0];
    let mut market = NationalMarket::new(&base_prices);
    let res = ResourceId::from(0);

    market.record_demand(res, 50.0);
    market.record_supply(res, 200.0);
    market.update_prices();

    let entry = market.get(res).expect("resource market entry should exist");
    assert!(
        entry.price < 100.0,
        "Price should drop when supply exceeds demand, got {}",
        entry.price
    );
    assert!((entry.sell_fulfillment - 0.25).abs() < 1e-6);
    assert_eq!(entry.buy_fulfillment, 1.0);
}

#[test]
fn test_market_corridor_clamping() {
    let base_prices = [100.0];
    let mut market = NationalMarket::new(&base_prices);
    let res = ResourceId::from(0);

    market.set_corridor(res, 95.0, 105.0);

    market.record_demand(res, 10_000.0);
    market.record_supply(res, 1.0);

    for _ in 0..50 {
        market.record_demand(res, 10_000.0);
        market.record_supply(res, 1.0);
        market.update_prices();
    }

    let price = market.price(res);
    assert!(
        price <= 105.0 + 1e-6,
        "Price {} must not exceed corridor upper bound 105.0",
        price
    );
    assert!(
        price >= 95.0 - 1e-6,
        "Price {} must not fall below corridor lower bound 95.0",
        price
    );
}

#[test]
fn test_market_custom_config() {
    let base_prices = [50.0];
    let mut market = NationalMarket::new(&base_prices);
    market.config = MarketConfig {
        elasticity: 0.8,
        smoothing: 0.5,
        ratio_min: 0.1,
        ratio_max: 10.0,
    };
    let res = ResourceId::from(0);
    market.record_demand(res, 200.0);
    market.record_supply(res, 50.0);
    market.update_prices();

    let entry = market.get(res).unwrap();
    assert!(entry.price > 50.0);
}
