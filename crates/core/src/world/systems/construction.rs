use crate::economy::finance::{BankLoan, BudgetSource};
use crate::economy::{
    CompanyState, FacilityId, FacilityState, Priority, ProductionLineId, ProductionLineState,
    RealEstateId, RealEstateProperty, Stockpile,
};
use crate::system::{System, SystemContext};
use crate::world::WorldCommand;
use crate::world::country::ConstructionTarget;
use crate::world::map::Region;
use crate::world::time::TimeTick;

pub struct ConstructionSystem;

impl System for ConstructionSystem {
    fn id(&self) -> &'static str {
        "core:construction_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_day()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let SystemContext {
            defs,
            countries,
            regions,
            companies,
            facilities,
            real_estate,
            commands,
            loans,
            ..
        } = ctx;

        let mut completed_targets = Vec::new();

        for country in countries.iter_mut() {
            let country_tag = country.tag;
            let market = &mut country.market;
            let treasury = &mut country.economy.treasury;

            country.construction_queue.retain_mut(|project| {
                let mut materials_daily_cost = 0.0;
                let mut min_fulfillment = 1.0f64;

                for &(res_id, amount) in &project.daily_materials {
                    if let Some(res_market) = market.get(res_id) {
                        let domestic_fulfillment = res_market.buy_fulfillment;
                        let global_avail = ctx
                            .global_market
                            .get(res_id)
                            .map(|g| g.global_surplus > 0.0 || g.daily_volume > 0.0)
                            .unwrap_or(false);
                        let effective_fulfillment = if domestic_fulfillment < 0.20 && global_avail {
                            0.75
                        } else {
                            domestic_fulfillment
                        };
                        min_fulfillment = min_fulfillment.min(effective_fulfillment);
                        materials_daily_cost += amount * res_market.price;
                    } else {
                        min_fulfillment = 0.0;
                    }
                }

                let total_req_workers: u32 = project.required_workers.iter().map(|(_, c)| *c).sum();
                let total_actual_workers: u32 =
                    project.current_workforce.iter().map(|(_, c)| *c).sum();

                let workforce_ratio = if total_req_workers > 0 {
                    (total_actual_workers as f64 / total_req_workers as f64).clamp(0.0, 1.0)
                } else {
                    1.0
                };

                let effective_speed = min_fulfillment.min(workforce_ratio);

                if effective_speed > 0.05 {
                    project.consecutive_stalled_days = 0;
                    let actual_fee = project.daily_contractor_fee * effective_speed;
                    let actual_materials_cost = materials_daily_cost * effective_speed;
                    let total_daily_cost = actual_fee + actual_materials_cost;

                    if Self::try_consume_payer(
                        &project.payer,
                        total_daily_cost,
                        country_tag,
                        treasury,
                        regions,
                        companies,
                        loans,
                    ) {
                        if let Some(contractor) =
                            companies.iter_mut().find(|c| c.id == project.contractor)
                        {
                            contractor.balance += actual_fee;
                            contractor.monthly_revenue += actual_fee;
                        }

                        for &(res_id, amount) in &project.daily_materials {
                            if let Some(res_market) = market.get_mut(res_id) {
                                res_market.revenue_pool +=
                                    (amount * effective_speed) * res_market.price;
                            }
                        }

                        project.progress += 1.0 * effective_speed;
                        project.last_effective_speed = effective_speed;
                    } else {
                        project.last_effective_speed = 0.0;
                        project.consecutive_stalled_days += 1;
                    }
                } else {
                    project.last_effective_speed = 0.0;
                    project.consecutive_stalled_days += 1;
                }

                if project.consecutive_stalled_days > 180 {
                    let project_region = match project.target {
                        ConstructionTarget::NewFacility { region_id, .. } => region_id,
                        ConstructionTarget::UpgradeStructure { region_id, .. } => region_id,
                        ConstructionTarget::NewRealEstate { region_id, .. } => region_id,
                    };
                    if let Some(region) = regions.iter_mut().find(|r| r.id == project_region) {
                        for &(pop_type_id, count) in &project.current_workforce {
                            if let Some(pop) = region
                                .pops
                                .iter_mut()
                                .find(|p| p.pop_type_id == pop_type_id)
                            {
                                pop.fire(count as u64);
                            }
                        }
                    }
                    return false;
                }

                if project.progress >= project.total_days {
                    let project_region = match project.target {
                        ConstructionTarget::NewFacility { region_id, .. } => region_id,
                        ConstructionTarget::UpgradeStructure { region_id, .. } => region_id,
                        ConstructionTarget::NewRealEstate { region_id, .. } => region_id,
                    };

                    if let Some(region) = regions.iter_mut().find(|r| r.id == project_region) {
                        for &(pop_type_id, count) in &project.current_workforce {
                            if let Some(pop) = region
                                .pops
                                .iter_mut()
                                .find(|p| p.pop_type_id == pop_type_id)
                            {
                                pop.fire(count as u64);
                            }
                        }
                    }

                    completed_targets.push((country_tag, project.target));
                    false
                } else {
                    true
                }
            });
        }

        for (country_tag, target) in completed_targets {
            match target {
                ConstructionTarget::NewFacility {
                    company_id,
                    recipe_id,
                    region_id,
                    sector_kind,
                } => {
                    let queued_count = commands
                        .iter()
                        .filter(|cmd| matches!(cmd, WorldCommand::SpawnFacility(_)))
                        .count() as u32;
                    let existing_max = facilities.iter().map(|f| f.id.0).max().unwrap_or(0);
                    let new_id =
                        FacilityId((existing_max + 1).max(facilities.len() as u32) + queued_count);

                    let base_wage = countries
                        .iter()
                        .find(|c| c.tag == country_tag)
                        .map(|c| c.economy.exchange_rate.max(1.0))
                        .unwrap_or(1.0);

                    let line = ProductionLineState {
                        id: ProductionLineId(0),
                        recipe_id,
                        installed_capacity: 10,
                        natural_max_capacity: Some(150),
                        target_capacity_cap: None,
                        utilization: 0.0,
                        potential_utilization: 1.0,
                        material_utilization: 0.0,
                        priority: Priority::Normal,
                        employed_workers: vec![],
                        target_wage: base_wage,
                        expansion_progress: 0.0,
                        last_day_output: 0.0,
                        input_shortage: vec![],
                        consecutive_idle_days: 0,
                    };

                    let initial_reserve = base_wage * 5000.0;
                    if let Some(comp) = companies.iter_mut().find(|c| c.id == company_id) {
                        let transfer = comp.balance.min(initial_reserve);
                        comp.balance -= transfer;
                    }

                    let mut stockpile = Stockpile::new(defs.resources.count());
                    if let Some(recipe) = defs.recipes.get(recipe_id) {
                        for &(res_id, req_per_cap) in &recipe.inputs {
                            stockpile.add(res_id, req_per_cap * 10.0 * recipe.safety_stock_days);
                        }
                    }

                    let new_facility = FacilityState {
                        id: new_id,
                        company_id,
                        country_tag,
                        sector_kind,
                        region_id,
                        stockpile,
                        lines: vec![line],
                        is_priority_development_zone: false,
                        local_reserve: initial_reserve,
                        pending_deliveries: Vec::new(),
                        period_revenue: 0.0,
                        period_material_cost: 0.0,
                        period_wage_cost: 0.0,
                    };

                    commands.push(WorldCommand::SpawnFacility(new_facility));
                }
                ConstructionTarget::UpgradeStructure {
                    region_id, def_id, ..
                } => {
                    if let Some(region) = regions.iter_mut().find(|r| r.id == region_id)
                        && let Some(structure) =
                            region.structures.iter_mut().find(|s| s.def_id == def_id)
                    {
                        structure.level += 1;
                    }
                }
                ConstructionTarget::NewRealEstate {
                    company_id,
                    kind,
                    region_id,
                    units,
                    base_rent,
                } => {
                    let queued_count = commands
                        .iter()
                        .filter(|cmd| matches!(cmd, WorldCommand::SpawnRealEstate(_)))
                        .count() as u32;
                    let existing_max = real_estate.iter().map(|r| r.id.0).max().unwrap_or(0);
                    let new_id = RealEstateId(
                        (existing_max + 1).max(real_estate.len() as u32) + queued_count,
                    );
                    let property = RealEstateProperty::new(
                        new_id,
                        kind,
                        company_id,
                        region_id,
                        country_tag,
                        units,
                        base_rent,
                    );
                    commands.push(WorldCommand::SpawnRealEstate(property));
                }
            }
        }
    }
}

impl ConstructionSystem {
    fn try_consume_payer(
        payer: &BudgetSource,
        amount: f64,
        current_country_tag: crate::CountryTag,
        treasury: &mut f64,
        regions: &mut [Region],
        companies: &mut [CompanyState],
        loans: &mut Vec<BankLoan>,
    ) -> bool {
        match *payer {
            BudgetSource::Federal(tag) if tag == current_country_tag => {
                if *treasury >= amount {
                    *treasury -= amount;
                    return true;
                }
            }
            BudgetSource::Regional(region_id) => {
                if let Some(r) = regions.iter_mut().find(|r| r.id == region_id)
                    && r.local_treasury >= amount
                {
                    r.local_treasury -= amount;
                    return true;
                }
            }
            BudgetSource::Company(company_id) => {
                if let Some(comp_idx) = companies.iter().position(|c| c.id == company_id) {
                    if companies[comp_idx].balance >= amount {
                        companies[comp_idx].balance -= amount;
                        return true;
                    }

                    let needed_loan = (amount * 30.0).max(10_000.0);
                    if BankLoan::request_corporate_loan(
                        comp_idx,
                        needed_loan,
                        current_country_tag,
                        companies,
                        loans,
                    ) && companies[comp_idx].balance >= amount
                    {
                        companies[comp_idx].balance -= amount;
                        return true;
                    }
                }
            }
            _ => {}
        }
        false
    }
}
