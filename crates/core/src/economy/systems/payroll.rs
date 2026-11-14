use crate::system::{System, SystemContext};
use crate::world::time::TimeTick;

#[derive(Default)]
pub struct PayrollSystem {
    company_funds: Vec<f64>,
    contractor_wage_expenses: Vec<f64>,
    pop_payouts: Vec<f64>,
    pop_stride: usize,
    country_regions: Vec<Vec<usize>>,
}

impl PayrollSystem {
    #[inline(always)]
    fn record_payout_to(
        payouts: &mut [f64],
        stride: usize,
        region_id: crate::world::map::RegionId,
        pop_type_id: crate::world::PopTypeId,
        amount: f64,
    ) {
        let idx = (region_id.0 as usize) * stride + (pop_type_id.0 as usize);
        if idx < payouts.len() {
            payouts[idx] += amount;
        }
    }
}

impl System for PayrollSystem {
    fn id(&self) -> &'static str {
        "core:payroll_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_day()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let max_id = ctx
            .companies
            .iter()
            .map(|c| c.id.0 as usize)
            .max()
            .unwrap_or(0);
        self.company_funds.clear();
        self.company_funds.resize(max_id + 1, 0.0);
        self.contractor_wage_expenses.clear();
        self.contractor_wage_expenses.resize(max_id + 1, 0.0);

        let max_reg_id = ctx
            .regions
            .iter()
            .map(|r| r.id.0 as usize)
            .max()
            .unwrap_or(0);
        let num_pop_types = ctx.defs.pop_types.len().max(1);
        self.pop_stride = num_pop_types;
        let total_cells = (max_reg_id + 1) * self.pop_stride;
        self.pop_payouts.clear();
        self.pop_payouts.resize(total_cells, 0.0);

        for c in ctx.companies.iter() {
            self.company_funds[c.id.0 as usize] = c.balance;
        }

        for facility in ctx.facilities.iter_mut() {
            let comp_id = facility.company_id.0 as usize;
            if comp_id >= self.company_funds.len() {
                continue;
            }
            let balance = &mut self.company_funds[comp_id];

            for line in &mut facility.lines {
                if line.utilization <= 0.0 || line.employed_workers.is_empty() {
                    continue;
                }

                for &(pop_type_id, workers_count) in &line.employed_workers {
                    if workers_count == 0 || line.target_wage <= 0.0 {
                        continue;
                    }

                    let wage_multiplier = ctx
                        .defs
                        .pop_types
                        .get(pop_type_id)
                        .map(|t| t.wage_multiplier)
                        .unwrap_or(1.0);

                    let effective_wage = line.target_wage * wage_multiplier;

                    let total_due = (workers_count as f64) * effective_wage * line.utilization;

                    let from_company = total_due.min((*balance).max(0.0));
                    *balance -= from_company;
                    let remaining = total_due - from_company;
                    let from_reserve = remaining.min(facility.local_reserve.max(0.0));
                    facility.local_reserve -= from_reserve;
                    let actual_payout = from_company + from_reserve;

                    if actual_payout > 0.0 {
                        facility.period_wage_cost += actual_payout;
                        Self::record_payout_to(
                            &mut self.pop_payouts,
                            self.pop_stride,
                            facility.region_id,
                            pop_type_id,
                            actual_payout,
                        );
                    }
                }
            }
        }

        for country in ctx.countries.iter() {
            let exchange_rate = country.economy.exchange_rate.max(0.001);
            for project in &country.construction_queue {
                if project.current_workforce.is_empty() || project.last_effective_speed <= 0.05 {
                    continue;
                }

                let project_region = match project.target {
                    crate::world::ConstructionTarget::NewFacility { region_id, .. } => region_id,
                    crate::world::ConstructionTarget::UpgradeStructure { region_id, .. } => {
                        region_id
                    }
                    crate::world::ConstructionTarget::NewRealEstate { region_id, .. } => region_id,
                };

                let contractor_id = project.contractor.0 as usize;
                if contractor_id >= self.company_funds.len() {
                    continue;
                }
                let balance = &mut self.company_funds[contractor_id];

                for &(pop_type_id, workers_count) in &project.current_workforce {
                    if workers_count == 0 {
                        continue;
                    }

                    let wage_multiplier = ctx
                        .defs
                        .pop_types
                        .get(pop_type_id)
                        .map(|t| t.wage_multiplier)
                        .unwrap_or(1.0);

                    let daily_due = (workers_count as f64)
                        * (1.2 * exchange_rate)
                        * wage_multiplier
                        * project.last_effective_speed;

                    let actual_payout = if *balance >= daily_due {
                        *balance -= daily_due;
                        daily_due
                    } else if *balance > 0.0 {
                        let partial = *balance;
                        *balance = 0.0;
                        partial
                    } else {
                        0.0
                    };

                    if actual_payout > 0.0 {
                        self.contractor_wage_expenses[contractor_id] += actual_payout;
                        Self::record_payout_to(
                            &mut self.pop_payouts,
                            self.pop_stride,
                            project_region,
                            pop_type_id,
                            actual_payout,
                        );
                    }
                }
            }
        }

        for company in ctx.companies.iter_mut() {
            let id = company.id.0 as usize;
            if id < self.contractor_wage_expenses.len() {
                let spent = self.contractor_wage_expenses[id];
                if spent > 0.0 {
                    company.monthly_expenses += spent;
                }
            }
            if id < self.company_funds.len() {
                company.balance = self.company_funds[id];
            }
        }

        for region in ctx.regions.iter_mut() {
            for structure in &mut region.structures {
                if structure.allocated_budget <= 0.0 || structure.current_workforce.is_empty() {
                    continue;
                }

                let remaining_budget = (structure.allocated_budget
                    - structure.period_maintenance_cost
                    - structure.period_wage_cost)
                    .max(0.0);
                if remaining_budget <= 0.0 {
                    continue;
                }

                for &(pop_type_id, workers_count) in &structure.current_workforce {
                    if workers_count == 0 || structure.wage_per_worker <= 0.0 {
                        continue;
                    }

                    let wage_multiplier = ctx
                        .defs
                        .pop_types
                        .get(pop_type_id)
                        .map(|t| t.wage_multiplier)
                        .unwrap_or(1.0);

                    let daily_due = (workers_count as f64)
                        * (structure.wage_per_worker * wage_multiplier / 30.0);

                    let actual_payout = daily_due.min(remaining_budget);
                    if actual_payout > 0.0 {
                        structure.period_wage_cost += actual_payout;
                        Self::record_payout_to(
                            &mut self.pop_payouts,
                            self.pop_stride,
                            region.id,
                            pop_type_id,
                            actual_payout,
                        );
                    }
                }
            }
        }

        for region in ctx.regions.iter_mut() {
            let reg_offset = (region.id.0 as usize) * self.pop_stride;
            for pop in &mut region.pops {
                let idx = reg_offset + (pop.pop_type_id.0 as usize);
                let earned = if idx < self.pop_payouts.len() {
                    self.pop_payouts[idx]
                } else {
                    0.0
                };
                if earned > 0.0 {
                    pop.employed.savings += earned;
                    pop.daily_income = earned;
                    pop.employed.period_income += earned;

                    if pop.employed.count > 0 {
                        pop.average_wage = earned / (pop.employed.count as f64);
                    }
                } else {
                    pop.daily_income = 0.0;
                }
            }
        }

        let num_countries = ctx.countries.len();
        if self.country_regions.len() < num_countries {
            self.country_regions.resize_with(num_countries, Vec::new);
        }
        for sub_vec in &mut self.country_regions[..num_countries] {
            sub_vec.clear();
        }

        for (idx, r) in ctx.regions.iter().enumerate() {
            if let Some(owner) = r.owner {
                if let Some(c_idx) = ctx.countries.iter().position(|c| c.tag == owner) {
                    self.country_regions[c_idx].push(idx);
                }
            }
        }

        for (c_idx, country) in ctx.countries.iter_mut().enumerate() {
            let reg_indices = &self.country_regions[c_idx];
            if reg_indices.is_empty() {
                continue;
            }

            let ex_rate = country.economy.exchange_rate.max(0.001);
            let welfare = country.economy.welfare;

            let mut total_beneficiaries = 0u64;
            for &r_idx in reg_indices {
                let region = &ctx.regions[r_idx];
                for pop in &region.pops {
                    let is_workforce = ctx
                        .defs
                        .pop_types
                        .get(pop.pop_type_id)
                        .map(|t| t.is_workforce)
                        .unwrap_or(true);
                    if is_workforce {
                        total_beneficiaries += pop.unemployed.count;
                    } else {
                        total_beneficiaries += pop.count();
                    }
                }
            }

            if total_beneficiaries == 0 || country.economy.treasury <= 10_000.0 * ex_rate {
                continue;
            }

            let daily_tax_share =
                (country.economy.last_period_tax_revenue * welfare.max_tax_share / 30.0).max(0.0);
            if daily_tax_share <= 0.0 {
                continue;
            }

            let max_daily_benefit = welfare.max_daily_benefit * ex_rate;
            let min_subsistence = welfare.min_daily_benefit * ex_rate;
            let benefit_per_person = (daily_tax_share / total_beneficiaries as f64)
                .clamp(min_subsistence, max_daily_benefit);

            if benefit_per_person <= 1e-6 {
                continue;
            }

            let mut total_paid = 0.0;
            for &r_idx in reg_indices {
                let region = &mut ctx.regions[r_idx];
                for pop in &mut region.pops {
                    let is_workforce = ctx
                        .defs
                        .pop_types
                        .get(pop.pop_type_id)
                        .map(|t| t.is_workforce)
                        .unwrap_or(true);
                    if is_workforce && pop.unemployed.count > 0 {
                        let amount = benefit_per_person * pop.unemployed.count as f64;
                        pop.unemployed.savings += amount;
                        pop.unemployed.period_income += amount;
                        pop.daily_income += amount;
                        total_paid += amount;
                    } else if !is_workforce && pop.count() > 0 {
                        let amount = benefit_per_person * pop.count() as f64;
                        pop.employed.savings += amount;
                        pop.employed.period_income += amount;
                        pop.daily_income += amount;
                        total_paid += amount;
                    }
                }
            }

            country.economy.treasury -= total_paid.min(country.economy.treasury);
        }
    }
}
