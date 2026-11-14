use engine_core::loader::DataLoader;
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
fn test_idle_unprofitable_facility_bankruptcy_and_closure() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let target_fac_id = world.facilities[0].id;
    let target_reg_id = world.facilities[0].region_id;
    let target_comp_id = world.facilities[0].company_id;

    let res_id = world.facilities[0]
        .stockpile
        .non_empty()
        .next()
        .map(|(r, _)| r);
    if let Some(r) = res_id {
        world.facilities[0].stockpile.add(r, 100.0);
    }

    world.facilities[0].local_reserve = 0.0;
    world.facilities[0].period_revenue = 0.0;
    world.facilities[0].period_wage_cost = 500.0;
    world.facilities[0].period_material_cost = 500.0;
    for line in &mut world.facilities[0].lines {
        line.consecutive_idle_days = 800;
        line.utilization = 0.0;
    }

    if let Some(comp) = world.companies.iter_mut().find(|c| c.id == target_comp_id) {
        comp.balance = -10_000.0;
        comp.last_month_profit = -500.0;
    }

    let initial_reg_stock = res_id
        .map(|r| {
            world
                .regions
                .iter()
                .find(|reg| reg.id == target_reg_id)
                .unwrap()
                .stockpile
                .get(r)
        })
        .unwrap_or(0.0);

    for _ in 0..31 {
        world.tick_day();
    }

    let still_exists = world.facilities.iter().any(|f| f.id == target_fac_id);
    assert!(!still_exists);

    let region = world
        .regions
        .iter()
        .find(|reg| reg.id == target_reg_id)
        .unwrap();
    assert!(!region.facilities.contains(&target_fac_id));

    if let Some(r) = res_id {
        let final_reg_stock = region.stockpile.get(r);
        assert!(final_reg_stock >= initial_reg_stock + 99.0);
    }
}

#[test]
fn test_profitable_facility_not_closed() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let initial_count = world.facilities.len();

    for _ in 0..31 {
        world.tick_day();
    }

    assert!(world.facilities.len() >= initial_count.saturating_sub(2));
}
