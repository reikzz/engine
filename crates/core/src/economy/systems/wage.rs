use crate::system::{System, SystemContext};
use crate::world::CountryTag;
use crate::world::time::TimeTick;
use std::collections::HashMap;

#[derive(Clone, Copy, Default)]
struct CountryWageRef {
    ex_rate: f64,
    base_ref_wage: f64,
    min_wage: f64,
    has_healthy_budget: bool,
    is_in_fiscal_stress: bool,
}

#[derive(Clone, Copy, Default)]
struct CompanyWageStatus {
    is_profitable: bool,
    is_struggling: bool,
    max_wage_cap: f64,
}

#[derive(Default)]
pub struct WageSystem {
    tag_to_country_idx: HashMap<CountryTag, usize>,
    country_wage_refs: Vec<CountryWageRef>,
    company_statuses: Vec<CompanyWageStatus>,
    company_workers: Vec<u32>,
    region_id_to_idx: Vec<usize>,
    region_commercial_wage_sum: Vec<f64>,
    region_commercial_wage_count: Vec<u32>,
}

impl System for WageSystem {
    fn id(&self) -> &'static str {
        "core:wage_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_month()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let SystemContext {
            defs,
            countries,
            regions,
            facilities,
            companies,
            ..
        } = ctx;

        self.tag_to_country_idx.clear();
        for (idx, country) in countries.iter().enumerate() {
            self.tag_to_country_idx.insert(country.tag, idx);
        }

        self.country_wage_refs.clear();
        self.country_wage_refs
            .resize(countries.len(), CountryWageRef::default());

        for (idx, country) in countries.iter().enumerate() {
            let ex_rate = country.economy.exchange_rate.max(0.001);
            let base_ref_wage = ex_rate.max(1.0);
            let min_wage = base_ref_wage * 0.4;
            let has_healthy_budget = country.economy.treasury > 50_000.0 * ex_rate;
            let is_in_fiscal_stress = country.economy.treasury < 10_000.0 * ex_rate;

            self.country_wage_refs[idx] = CountryWageRef {
                ex_rate,
                base_ref_wage,
                min_wage,
                has_healthy_budget,
                is_in_fiscal_stress,
            };
        }

        let max_comp_id = companies.iter().map(|c| c.id.0 as usize).max().unwrap_or(0);
        self.company_workers.clear();
        self.company_workers.resize(max_comp_id + 1, 0);

        for facility in facilities.iter() {
            let comp_id = facility.company_id.0 as usize;
            if comp_id < self.company_workers.len() {
                for line in &facility.lines {
                    for (_, count) in &line.employed_workers {
                        self.company_workers[comp_id] += *count;
                    }
                }
            }
        }

        self.company_statuses.clear();
        self.company_statuses
            .resize(max_comp_id + 1, CompanyWageStatus::default());

        for comp in companies.iter() {
            let comp_idx = comp.id.0 as usize;
            let ex_rate = self
                .tag_to_country_idx
                .get(&comp.country_tag)
                .map(|&idx| self.country_wage_refs[idx].ex_rate)
                .unwrap_or(1.0);
            let base_ref_wage = ex_rate.max(1.0);
            let min_reserve = 5_000.0 * ex_rate;
            let is_profitable = comp.last_month_profit > 0.0 && comp.balance > min_reserve;
            let is_struggling =
                comp.balance < min_reserve || comp.last_month_profit < -100.0 * ex_rate;

            let workers = self.company_workers[comp_idx];
            let mut max_wage_cap = base_ref_wage * 4.0;

            if is_profitable && workers > 0 {
                let workers_f = workers as f64;
                let daily_rev_per_worker = (comp.monthly_revenue.max(0.0) / 30.0) / workers_f;
                let daily_profit_per_worker = (comp.last_month_profit.max(0.0) / 30.0) / workers_f;
                let affordable_profit_share = base_ref_wage * 4.0 + daily_profit_per_worker * 0.45;
                let affordable_rev_share = daily_rev_per_worker * 0.50;
                let dynamic_cap = affordable_profit_share.max(affordable_rev_share);
                max_wage_cap = max_wage_cap.max(dynamic_cap).min(base_ref_wage * 50.0);
            }

            self.company_statuses[comp_idx] = CompanyWageStatus {
                is_profitable,
                is_struggling,
                max_wage_cap,
            };
        }

        let max_region_id = regions.iter().map(|r| r.id.0 as usize).max().unwrap_or(0);
        self.region_id_to_idx.clear();
        self.region_id_to_idx.resize(max_region_id + 1, usize::MAX);

        for (idx, region) in regions.iter().enumerate() {
            self.region_id_to_idx[region.id.0 as usize] = idx;
        }

        self.region_commercial_wage_sum.clear();
        self.region_commercial_wage_sum.resize(regions.len(), 0.0);
        self.region_commercial_wage_count.clear();
        self.region_commercial_wage_count.resize(regions.len(), 0);

        for facility in facilities.iter_mut() {
            let Some(&country_idx) = self.tag_to_country_idx.get(&facility.country_tag) else {
                continue;
            };
            let country_ref = self.country_wage_refs[country_idx];

            let comp_id = facility.company_id.0 as usize;
            let (is_profitable, is_struggling, max_wage_cap) = if comp_id
                < self.company_statuses.len()
                && self.company_statuses[comp_id].max_wage_cap > 0.0
            {
                let status = self.company_statuses[comp_id];
                (
                    status.is_profitable,
                    status.is_struggling,
                    status.max_wage_cap.max(country_ref.min_wage),
                )
            } else {
                (
                    false,
                    false,
                    (country_ref.base_ref_wage * 4.0).max(country_ref.min_wage),
                )
            };

            let reg_id = facility.region_id.0 as usize;
            if reg_id >= self.region_id_to_idx.len() {
                continue;
            }
            let reg_idx = self.region_id_to_idx[reg_id];
            if reg_idx == usize::MAX || reg_idx >= regions.len() {
                continue;
            }
            let region = &regions[reg_idx];

            for line in &mut facility.lines {
                if line.installed_capacity == 0 {
                    continue;
                }

                let Some(recipe) = defs.recipes.get(line.recipe_id) else {
                    continue;
                };

                let mut avg_unemployment = 0.0;
                let mut req_types_count = 0;

                for &(pop_type_id, _) in &recipe.workforce_requirements {
                    if let Some(pop) = region.pops.iter().find(|p| p.pop_type_id == pop_type_id) {
                        let total = pop.count();
                        let unemp = pop.free_workforce();
                        let rate = if total > 0 {
                            (unemp as f64) / (total as f64)
                        } else {
                            0.0
                        };
                        avg_unemployment += rate;
                        req_types_count += 1;
                    }
                }

                if req_types_count > 0 {
                    avg_unemployment /= req_types_count as f64;
                }

                let mut wage_mult = 1.0;

                if is_profitable {
                    if avg_unemployment < 0.05 {
                        wage_mult += 0.06;
                    } else if avg_unemployment < 0.12 {
                        wage_mult += 0.04;
                    } else {
                        wage_mult += 0.02;
                    }
                } else if is_struggling {
                    if line.consecutive_idle_days > 10 || line.utilization < 0.4 {
                        wage_mult -= 0.03;
                    } else {
                        wage_mult -= 0.01;
                    }
                } else if avg_unemployment > 0.25 {
                    wage_mult -= 0.005;
                }

                let safe_max = max_wage_cap.max(country_ref.min_wage);
                line.target_wage =
                    (line.target_wage * wage_mult).clamp(country_ref.min_wage, safe_max);

                self.region_commercial_wage_sum[reg_idx] += line.target_wage;
                self.region_commercial_wage_count[reg_idx] += 1;
            }
        }

        for (reg_idx, region) in regions.iter_mut().enumerate() {
            let Some(country_tag) = region.owner else {
                continue;
            };
            let Some(&country_idx) = self.tag_to_country_idx.get(&country_tag) else {
                continue;
            };
            let country_ref = self.country_wage_refs[country_idx];
            let base_ref_monthly = country_ref.base_ref_wage * 30.0;
            let min_wage = base_ref_monthly * 0.4;

            let count = self.region_commercial_wage_count[reg_idx];
            let avg_commercial_monthly = if count > 0 {
                (self.region_commercial_wage_sum[reg_idx] / (count as f64)) * 30.0
            } else {
                base_ref_monthly
            };

            let max_wage = (base_ref_monthly * 3.0)
                .max(avg_commercial_monthly * 1.1)
                .min(base_ref_monthly * 50.0);

            for structure in &mut region.structures {
                let Some(def) = defs.structures.get(structure.def_id) else {
                    continue;
                };
                let level_mult = structure.level as f64;
                let mut has_unfilled = false;

                for &(pop_type_id, req_per_level) in &def.workforce_requirements {
                    let target = ((req_per_level as f64) * level_mult).ceil() as u32;
                    let current = structure
                        .current_workforce
                        .iter()
                        .find(|&&(id, _)| id == pop_type_id)
                        .map(|&(_, c)| c)
                        .unwrap_or(0);
                    if current < target {
                        has_unfilled = true;
                    }
                }

                if country_ref.has_healthy_budget {
                    if structure.wage_per_worker < avg_commercial_monthly * 0.95 {
                        structure.wage_per_worker =
                            (structure.wage_per_worker * 1.05).clamp(min_wage, max_wage);
                    } else if has_unfilled {
                        structure.wage_per_worker =
                            (structure.wage_per_worker * 1.02).clamp(min_wage, max_wage);
                    }
                } else if country_ref.is_in_fiscal_stress {
                    structure.wage_per_worker =
                        (structure.wage_per_worker * 0.99).clamp(min_wage, max_wage);
                }
            }
        }
    }
}
