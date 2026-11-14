use crate::CountryTag;
use crate::economy::real_estate::RealEstateKind;
use crate::economy::sector::SectorKind;
use crate::system::{System, SystemContext};
use crate::world::map::{RegionId, RegionModifier};
use crate::world::time::TimeTick;
use std::collections::HashMap;

#[derive(Default)]
pub struct RealEstateSystem {
    country_tags: Vec<CountryTag>,
    exchange_rates: Vec<f64>,
}

impl System for RealEstateSystem {
    fn id(&self) -> &'static str {
        "core:real_estate_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_month()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        self.country_tags.clear();
        self.exchange_rates.clear();
        for c in ctx.countries.iter() {
            self.country_tags.push(c.tag);
            self.exchange_rates
                .push(c.economy.exchange_rate.max(0.0001));
        }

        let mut region_office_workers: HashMap<RegionId, u32> = HashMap::new();
        for facility in ctx.facilities.iter() {
            if facility.sector_kind == SectorKind::InformationTechnology
                || facility.sector_kind == SectorKind::Financial
            {
                let mut facility_workers = 0u32;
                for line in &facility.lines {
                    for &(_, count) in &line.employed_workers {
                        facility_workers += count;
                    }
                }
                *region_office_workers.entry(facility.region_id).or_insert(0) += facility_workers;
            }
        }

        for region in ctx.regions.iter() {
            let mut struct_office_workers = 0u32;
            for st in &region.structures {
                struct_office_workers += (st.level as u32) * 150;
            }
            *region_office_workers.entry(region.id).or_insert(0) += struct_office_workers;
        }

        let mut region_office_capacity: HashMap<RegionId, u32> = HashMap::new();
        let mut region_housing_capacity: HashMap<RegionId, u32> = HashMap::new();
        for prop in ctx.real_estate.iter() {
            match prop.kind {
                RealEstateKind::CommercialOffice => {
                    *region_office_capacity.entry(prop.region_id).or_insert(0) += prop.total_units;
                }
                RealEstateKind::ResidentialHousing => {
                    *region_housing_capacity.entry(prop.region_id).or_insert(0) += prop.total_units;
                }
            }
        }

        let mut region_population: HashMap<RegionId, u64> = HashMap::new();
        for region in ctx.regions.iter() {
            let pop_sum: u64 = region.pops.iter().map(|p| p.count()).sum();
            region_population.insert(region.id, pop_sum);
        }

        let mut region_housing_rent_collected: HashMap<RegionId, f64> = HashMap::new();

        for prop in ctx.real_estate.iter_mut() {
            match prop.kind {
                RealEstateKind::CommercialOffice => {
                    let total_workers = region_office_workers
                        .get(&prop.region_id)
                        .copied()
                        .unwrap_or(0);
                    let total_cap = region_office_capacity
                        .get(&prop.region_id)
                        .copied()
                        .unwrap_or(0);

                    if total_cap > 0 {
                        let share = prop.total_units as f64 / total_cap as f64;
                        let allocated = (total_workers as f64 * share).round() as u32;
                        prop.occupied_units = allocated.min(prop.total_units);
                    } else {
                        prop.occupied_units = 0;
                    }

                    prop.update_rent_price();

                    let revenue = (prop.occupied_units as f64) * prop.current_rent_per_unit;
                    let maintenance = (prop.total_units as f64) * prop.base_rent_per_unit * 0.15;

                    prop.period_revenue = revenue;
                    prop.period_maintenance_cost = maintenance;

                    if let Some(comp_id) = prop.owner_company_id {
                        if let Some(comp) = ctx.companies.iter_mut().find(|c| c.id == comp_id) {
                            let net_income = revenue - maintenance;
                            comp.balance += net_income;
                            comp.monthly_revenue += revenue.max(0.0);
                            comp.monthly_expenses += maintenance.max(0.0);
                        }
                    }
                }
                RealEstateKind::ResidentialHousing => {
                    let total_pop = region_population.get(&prop.region_id).copied().unwrap_or(0);
                    let total_cap = region_housing_capacity
                        .get(&prop.region_id)
                        .copied()
                        .unwrap_or(0);

                    let household_demand = (total_pop as f64 / 2.5).round() as u32;

                    if total_cap > 0 {
                        let share = prop.total_units as f64 / total_cap as f64;
                        let allocated = (household_demand as f64 * share).round() as u32;
                        prop.occupied_units = allocated.min(prop.total_units);
                    } else {
                        prop.occupied_units = 0;
                    }

                    prop.update_rent_price();

                    let revenue = (prop.occupied_units as f64) * prop.current_rent_per_unit;
                    let maintenance = (prop.total_units as f64) * prop.base_rent_per_unit * 0.12;

                    prop.period_revenue = revenue;
                    prop.period_maintenance_cost = maintenance;

                    *region_housing_rent_collected
                        .entry(prop.region_id)
                        .or_insert(0.0) += revenue;

                    if let Some(comp_id) = prop.owner_company_id {
                        if let Some(comp) = ctx.companies.iter_mut().find(|c| c.id == comp_id) {
                            let net_income = revenue - maintenance;
                            comp.balance += net_income;
                            comp.monthly_revenue += revenue.max(0.0);
                            comp.monthly_expenses += maintenance.max(0.0);
                        }
                    }
                }
            }
        }

        for region in ctx.regions.iter_mut() {
            let total_housing_cap = region_housing_capacity
                .get(&region.id)
                .copied()
                .unwrap_or(0);
            let total_pop = region.population().max(1);
            let household_demand = ((total_pop as f64) / 2.5).round() as u64;

            let coverage = if household_demand > 0 {
                (total_housing_cap as f64 / household_demand as f64).clamp(0.0, 3.0)
            } else {
                1.0
            };

            if coverage >= 0.95 {
                let growth_bonus = ((coverage - 0.95) * 0.15).min(0.12);
                region
                    .modifiers
                    .add(RegionModifier::PopulationGrowth, growth_bonus);
                region.modifiers.add(RegionModifier::SocialStability, 0.05);
            } else {
                let penalty = ((0.95 - coverage) * 0.22).min(0.25);
                region
                    .modifiers
                    .add(RegionModifier::PopulationGrowth, -penalty);
                region
                    .modifiers
                    .add(RegionModifier::SocialStability, -penalty * 0.6);
            }

            if let Some(&rent_collected) = region_housing_rent_collected.get(&region.id) {
                if rent_collected > 0.0 {
                    let rent_per_person = rent_collected / (total_pop as f64);
                    for pop in &mut region.pops {
                        let emp_deduction = (pop.employed.count as f64) * rent_per_person;
                        let unemp_deduction = (pop.unemployed.count as f64) * rent_per_person * 0.5;

                        pop.employed.savings = (pop.employed.savings - emp_deduction).max(0.0);
                        pop.unemployed.savings =
                            (pop.unemployed.savings - unemp_deduction).max(0.0);

                        if coverage < 0.85 {
                            pop.satisfaction = (pop.satisfaction - 0.06).clamp(0.05, 1.0);
                        } else if coverage >= 1.0 {
                            pop.satisfaction = (pop.satisfaction + 0.03).clamp(0.05, 1.0);
                        }
                    }
                }
            }
        }

        let mut office_synergy: HashMap<RegionId, f64> = HashMap::new();
        for prop in ctx.real_estate.iter() {
            if prop.kind == RealEstateKind::CommercialOffice {
                let scale = (prop.occupied_units as f64 / 2500.0).clamp(0.0, 4.0);
                *office_synergy.entry(prop.region_id).or_insert(0.0) += 0.05 * scale;
            }
        }

        for region in ctx.regions.iter_mut() {
            if let Some(&bonus) = office_synergy.get(&region.id) {
                region
                    .modifiers
                    .add(RegionModifier::LaborProductivity, bonus);
                region
                    .modifiers
                    .add(RegionModifier::MarketAccess, bonus * 0.5);
            }
        }

        self.reinvest_developer_profits(ctx);
    }
}

impl RealEstateSystem {
    fn reinvest_developer_profits(&self, ctx: &mut SystemContext) {
        for prop in ctx.real_estate.iter_mut() {
            if prop.occupancy_ratio() < 0.90 {
                continue;
            }

            let Some(comp_id) = prop.owner_company_id else {
                continue;
            };

            let Some(comp) = ctx.companies.iter_mut().find(|c| c.id == comp_id) else {
                continue;
            };

            if comp.sector_kind != SectorKind::RealEstate {
                continue;
            }

            let ex_rate = self
                .country_tags
                .iter()
                .position(|t| *t == comp.country_tag)
                .map(|idx| self.exchange_rates[idx])
                .unwrap_or(1.0);

            let expansion_units = 10_000u32;
            let build_cost = (expansion_units as f64) * prop.base_rent_per_unit * 1.5;

            if comp.balance >= build_cost && comp.balance > 2_000_000.0 * ex_rate {
                comp.balance -= build_cost;
                comp.monthly_expenses += build_cost;
                prop.total_units += expansion_units;
            }
        }
    }
}
