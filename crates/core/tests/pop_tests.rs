use engine_core::world::{Pop, PopTypeId};

#[test]
fn test_pop_creation() {
    let pop_type = PopTypeId(0);
    let pop = Pop::new(pop_type, 10_000);

    assert_eq!(pop.pop_type_id, pop_type);
    assert_eq!(pop.count(), 10_000);
    assert_eq!(pop.free_workforce(), 10_000);
    assert_eq!(pop.employed.count, 0);
    assert_eq!(pop.unemployed.count, 10_000);
    assert_eq!(pop.total_savings(), 0.0);
    assert_eq!(pop.savings_per_capita(), 0.0);
}

#[test]
fn test_pop_hire_transfers_workforce_and_capital() {
    let mut pop = Pop::new(PopTypeId(1), 1_000);
    pop.unemployed.savings = 5_000.0;

    pop.hire(200);

    assert_eq!(pop.employed.count, 200);
    assert_eq!(pop.unemployed.count, 800);
    assert_eq!(pop.free_workforce(), 800);
    assert_eq!(pop.count(), 1_000);

    assert!((pop.employed.savings - 1_000.0).abs() < 1e-6);
    assert!((pop.unemployed.savings - 4_000.0).abs() < 1e-6);
    assert!((pop.total_savings() - 5_000.0).abs() < 1e-6);
    assert!((pop.savings_per_capita() - 5.0).abs() < 1e-6);
}

#[test]
fn test_pop_hire_clamped_to_available_unemployed() {
    let mut pop = Pop::new(PopTypeId(0), 100);
    pop.unemployed.savings = 1_000.0;

    pop.hire(250);

    assert_eq!(pop.employed.count, 100);
    assert_eq!(pop.unemployed.count, 0);
    assert_eq!(pop.free_workforce(), 0);
    assert_eq!(pop.employed.savings, 1_000.0);
    assert_eq!(pop.unemployed.savings, 0.0);
}

#[test]
fn test_pop_fire_transfers_workforce_and_capital_back() {
    let mut pop = Pop::new(PopTypeId(2), 1_000);
    pop.unemployed.savings = 10_000.0;
    pop.hire(1_000);

    pop.employed.savings += 2_000.0;

    pop.fire(250);

    assert_eq!(pop.employed.count, 750);
    assert_eq!(pop.unemployed.count, 250);

    assert!((pop.unemployed.savings - 3_000.0).abs() < 1e-6);
    assert!((pop.employed.savings - 9_000.0).abs() < 1e-6);
    assert!((pop.total_savings() - 12_000.0).abs() < 1e-6);
}

#[test]
fn test_pop_empty_transfer_edge_cases() {
    let mut pop = Pop::new(PopTypeId(0), 0);
    pop.hire(50);
    assert_eq!(pop.employed.count, 0);
    assert_eq!(pop.unemployed.count, 0);

    pop.fire(50);
    assert_eq!(pop.employed.count, 0);
    assert_eq!(pop.unemployed.count, 0);
    assert_eq!(pop.savings_per_capita(), 0.0);
}

#[test]
fn test_pop_debt_transfer_on_hire_and_fire() {
    let mut pop = Pop::new(PopTypeId(0), 1_000);
    pop.unemployed.debt = 2_000.0;
    assert_eq!(pop.total_debt(), 2_000.0);
    assert_eq!(pop.debt_per_capita(), 2.0);

    pop.hire(400);
    assert_eq!(pop.employed.count, 400);
    assert_eq!(pop.unemployed.count, 600);
    assert!((pop.employed.debt - 800.0).abs() < 1e-6);
    assert!((pop.unemployed.debt - 1200.0).abs() < 1e-6);

    pop.fire(100);
    assert_eq!(pop.employed.count, 300);
    assert_eq!(pop.unemployed.count, 700);
    assert!((pop.employed.debt - 600.0).abs() < 1e-6);
    assert!((pop.unemployed.debt - 1400.0).abs() < 1e-6);
    assert_eq!(pop.debt_per_capita(), 2.0);
}
