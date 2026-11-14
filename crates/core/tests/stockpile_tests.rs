use engine_core::economy::{ResourceId, Stockpile};

#[test]
fn test_stockpile_initialization() {
    let stock = Stockpile::new(5);
    for i in 0..5 {
        assert_eq!(stock.get(ResourceId::from(i)), 0.0);
    }
    assert_eq!(stock.get(ResourceId::from(99)), 0.0);
}

#[test]
fn test_stockpile_add_and_get() {
    let mut stock = Stockpile::new(3);
    let res0 = ResourceId::from(0);
    let res1 = ResourceId::from(1);

    stock.add(res0, 50.0);
    stock.add(res0, 25.5);
    stock.add(res1, 10.0);

    assert_eq!(stock.get(res0), 75.5);
    assert_eq!(stock.get(res1), 10.0);
    assert_eq!(stock.get(ResourceId::from(2)), 0.0);
}

#[test]
fn test_stockpile_try_consume_success() {
    let mut stock = Stockpile::new(2);
    let res = ResourceId::from(0);

    stock.add(res, 100.0);
    let ok = stock.try_consume(res, 40.0);

    assert!(ok);
    assert_eq!(stock.get(res), 60.0);

    let ok_exact = stock.try_consume(res, 60.0);
    assert!(ok_exact);
    assert_eq!(stock.get(res), 0.0);
}

#[test]
fn test_stockpile_try_consume_insufficient_fails_atomically() {
    let mut stock = Stockpile::new(2);
    let res = ResourceId::from(0);

    stock.add(res, 50.0);
    let ok = stock.try_consume(res, 75.0);

    assert!(!ok);
    assert_eq!(stock.get(res), 50.0);
}

#[test]
fn test_stockpile_try_consume_zero_or_negative() {
    let mut stock = Stockpile::new(2);
    let res = ResourceId::from(0);

    stock.add(res, 10.0);
    assert!(stock.try_consume(res, 0.0));
    assert!(stock.try_consume(res, -5.0));
    assert_eq!(stock.get(res), 10.0);
}

#[test]
fn test_stockpile_non_empty_iterator() {
    let mut stock = Stockpile::new(4);
    let res1 = ResourceId::from(1);
    let res3 = ResourceId::from(3);

    stock.add(res1, 15.0);
    stock.add(res3, 42.0);

    let non_empty: Vec<(ResourceId, f64)> = stock.non_empty().collect();
    assert_eq!(non_empty.len(), 2);
    assert_eq!(non_empty[0], (res1, 15.0));
    assert_eq!(non_empty[1], (res3, 42.0));
}
