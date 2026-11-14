use engine_core::CountryTag;
use engine_core::economy::{Ownership, Priority, SectorKind};
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
fn test_tax_and_welfare_policy_actions() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let tag = CountryTag::new("USA");
    world.set_tax_rates(tag, 0.25, 0.18, 0.06).unwrap();

    let country = world.country(tag).unwrap();
    assert_eq!(country.economy.corporate_tax, 0.25);
    assert_eq!(country.economy.income_tax, 0.18);
    assert_eq!(country.economy.tariff_rate, 0.06);

    world
        .set_bailout_policy(tag, BailoutPolicy::Universal, 0.12)
        .unwrap();
    let country_after_bailout = world.country(tag).unwrap();
    assert_eq!(
        country_after_bailout.economy.bailout_policy,
        BailoutPolicy::Universal
    );
    assert_eq!(country_after_bailout.economy.max_subsidy_budget_ratio, 0.12);
}

#[test]
fn test_debt_and_emergency_welfare() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let tag = CountryTag::new("USA");
    let init_treasury = world.country(tag).unwrap().economy.treasury;
    let init_debt = world.country(tag).unwrap().economy.national_debt;

    world.take_national_loan(tag, 50_000.0).unwrap();
    let country = world.country(tag).unwrap();
    assert_eq!(country.economy.treasury, init_treasury + 50_000.0);
    assert_eq!(country.economy.national_debt, init_debt + 50_000.0);

    world.repay_national_debt(tag, 20_000.0).unwrap();
    let country_repaid = world.country(tag).unwrap();
    assert_eq!(country_repaid.economy.national_debt, init_debt + 30_000.0);

    let spent = world.issue_emergency_welfare(tag, 10_000.0).unwrap();
    assert!(spent > 0.0);
}

#[test]
fn test_company_and_facility_actions() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let tag = CountryTag::new("USA");
    let new_comp_id = world
        .found_state_company(
            tag,
            "US Federal Logistics".to_string(),
            SectorKind::Consumer,
            10_000.0,
        )
        .unwrap();

    let comp = world
        .companies
        .iter()
        .find(|c| c.id == new_comp_id)
        .unwrap();
    assert_eq!(comp.ownership, Ownership::State);
    assert_eq!(comp.balance, 10_000.0);

    world
        .transfer_treasury_to_company(tag, new_comp_id, 5_000.0)
        .unwrap();
    let comp_funded = world
        .companies
        .iter()
        .find(|c| c.id == new_comp_id)
        .unwrap();
    assert_eq!(comp_funded.balance, 15_000.0);

    world.privatize_company(tag, new_comp_id, 20_000.0).unwrap();
    let comp_privatized = world
        .companies
        .iter()
        .find(|c| c.id == new_comp_id)
        .unwrap();
    assert!(matches!(
        comp_privatized.ownership,
        Ownership::Private { .. }
    ));

    let fac_id = world.facilities[0].id;
    world.set_facility_priority_zone(fac_id, true).unwrap();
    assert!(world.facilities[0].is_priority_development_zone);

    world
        .set_production_line_priority(fac_id, 0, Priority::Critical)
        .unwrap();
    assert_eq!(world.facilities[0].lines[0].priority, Priority::Critical);

    world
        .set_production_line_capacity_cap(fac_id, 0, Some(50))
        .unwrap();
    assert_eq!(world.facilities[0].lines[0].target_capacity_cap, Some(50));
}

#[test]
fn test_commission_and_cancel_construction() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let tag = CountryTag::new("USA");
    let company_id = world
        .companies
        .iter()
        .find(|c| c.country_tag == tag)
        .map(|c| c.id)
        .unwrap();

    let recipe_id = world.defs.recipes.iter().next().unwrap().0;
    let region_id = world.regions[0].id;

    let init_queue_len = world.country(tag).unwrap().construction_queue.len();
    world
        .commission_facility_construction(tag, company_id, recipe_id, region_id, None)
        .unwrap();

    let new_queue_len = world.country(tag).unwrap().construction_queue.len();
    assert_eq!(new_queue_len, init_queue_len + 1);

    world
        .cancel_construction_project(tag, new_queue_len - 1)
        .unwrap();
    let cancelled_queue_len = world.country(tag).unwrap().construction_queue.len();
    assert_eq!(cancelled_queue_len, init_queue_len);
}

#[test]
fn test_construction_and_upgrade_cost_estimation() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let tag = CountryTag::new("USA");
    let recipe_id = world.defs.recipes.iter().next().unwrap().0;

    let facility_estimate = world
        .calculate_facility_construction_cost(tag, recipe_id)
        .unwrap();
    assert!(facility_estimate.build_days >= 30.0);
    assert!(facility_estimate.mobilization_advance > 0.0);
    assert!(facility_estimate.total_estimated_cost > 0.0);
    assert!(facility_estimate.daily_contractor_fee > 0.0);
    assert_eq!(facility_estimate.recipe_id, recipe_id);

    let structure_def_id = world.defs.structures.iter().next().unwrap().0;
    let structure_estimate = world
        .calculate_structure_upgrade_cost(tag, structure_def_id)
        .unwrap();
    assert!(structure_estimate.build_days >= 30.0);
    assert!(structure_estimate.mobilization_advance > 0.0);
    assert!(structure_estimate.total_estimated_cost > 0.0);
    assert_eq!(structure_estimate.structure_def_id, structure_def_id);
}

#[test]
fn test_region_detailed_inspection() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let region_id = world.regions[0].id;
    let inspection = world.inspect_region(region_id).unwrap();

    assert_eq!(inspection.region_id, region_id);
    assert!(inspection.total_population > 0);
    assert!(inspection.unemployment_rate >= 0.0 && inspection.unemployment_rate <= 1.0);
    assert_eq!(
        inspection.facilities.len(),
        world.regions[0].facilities.len()
    );
    assert_eq!(
        inspection.structures.len(),
        world.regions[0].structures.len()
    );
}

#[test]
fn test_technologies_overview_query() {
    let data_dir = find_data_dir();
    let defs = DataLoader::load_game_defs(&data_dir.join("common")).unwrap();
    let mut world = DataLoader::load_world(&data_dir.join("scenarios/2020_default"), defs).unwrap();

    let tag = CountryTag::new("USA");
    let overview = world.get_technologies_overview(tag).unwrap();
    assert!(!overview.is_empty());
    assert_eq!(overview.len(), world.defs.technologies.len());

    let active_candidate = overview
        .iter()
        .find(|t| t.status == engine_core::world::TechnologyResearchStatus::Available);
    if let Some(cand) = active_candidate {
        world.set_active_research(tag, cand.id).unwrap();
        let active = world.get_active_research_overview(tag);
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, cand.id);
    }
}
