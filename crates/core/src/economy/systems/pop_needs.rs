use crate::CountryTag;
use crate::economy::market::NationalMarket;
use crate::system::{System, SystemContext};
use crate::world::time::TimeTick;
use crate::world::{PopNeed, PopSubgroup};

#[derive(Default)]
pub struct PopNeedsSystem {
    country_tags: Vec<CountryTag>,
    country_demands: Vec<Vec<f64>>,
    region_country_indices: Vec<usize>,
    cached_needs: Vec<Vec<PopNeed>>,
}

impl System for PopNeedsSystem {
    fn id(&self) -> &'static str {
        "core:pop_needs_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_day()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let num_countries = ctx.countries.len();
        let num_resources = ctx.defs.resources.len();

        self.country_tags.clear();
        for c in ctx.countries.iter() {
            self.country_tags.push(c.tag);
        }

        self.region_country_indices.clear();
        self.region_country_indices
            .resize(ctx.regions.len(), usize::MAX);
        for (r_idx, reg) in ctx.regions.iter().enumerate() {
            if let Some(owner) = reg.owner {
                if let Some(pos) = self.country_tags.iter().position(|t| *t == owner) {
                    self.region_country_indices[r_idx] = pos;
                }
            }
        }

        if self.cached_needs.is_empty() {
            self.cached_needs
                .resize(ctx.defs.pop_types.len(), Vec::new());
            for (pop_type_id, pop_type) in ctx.defs.pop_types.iter() {
                let idx = pop_type_id.0 as usize;
                if idx >= self.cached_needs.len() {
                    self.cached_needs.resize(idx + 1, Vec::new());
                }
                self.cached_needs[idx] = pop_type.needs.clone();
            }
        }

        if self.country_demands.len() != num_countries {
            self.country_demands
                .resize_with(num_countries, || vec![0.0; num_resources]);
        }

        for demands in &mut self.country_demands {
            if demands.len() != num_resources {
                demands.resize(num_resources, 0.0);
            }
            demands.fill(0.0);
        }

        for (r_idx, region) in ctx.regions.iter_mut().enumerate() {
            let c_idx = self.region_country_indices[r_idx];
            if c_idx == usize::MAX {
                continue;
            }

            let market = &ctx.countries[c_idx].market;
            let demands_for_country = &mut self.country_demands[c_idx];
            let market_access = region.market_access;

            for pop in &region.pops {
                let pop_type_idx = pop.pop_type_id.0 as usize;
                let pop_needs = if pop_type_idx < self.cached_needs.len() {
                    self.cached_needs[pop_type_idx].as_slice()
                } else {
                    &[]
                };

                if pop_needs.is_empty() {
                    continue;
                }

                Self::subgroup_demand(
                    &pop.employed,
                    pop_needs,
                    market,
                    market_access,
                    demands_for_country,
                    &mut region.local_demanded,
                );
                Self::subgroup_demand(
                    &pop.unemployed,
                    pop_needs,
                    market,
                    market_access,
                    demands_for_country,
                    &mut region.local_demanded,
                );
            }
        }

        for (c_idx, country) in ctx.countries.iter_mut().enumerate() {
            let demands = &self.country_demands[c_idx];
            for (res_id, _) in ctx.defs.resources.iter() {
                let res_idx = res_id.0 as usize;
                if res_idx < demands.len() {
                    let amount = demands[res_idx];
                    if amount > 0.0 {
                        country.market.record_demand(res_id, amount);
                    }
                }
            }
        }
    }
}

impl PopNeedsSystem {
    #[inline]
    pub fn subgroup_demand(
        subgroup: &PopSubgroup,
        needs: &[PopNeed],
        market: &NationalMarket,
        market_access: f64,
        demands_out: &mut [f64],
        local_demands_out: &mut [f64],
    ) {
        if subgroup.count == 0 || subgroup.savings <= 0.0 {
            return;
        }

        let mut budget_remaining = subgroup.savings;
        let savings_per_capita = (subgroup.savings / (subgroup.count as f64).max(1.0)).max(0.0);

        let comfort_mult = if savings_per_capita > 50.0 {
            (1.0 + (savings_per_capita / 50.0).log2()).clamp(1.0, 4.0)
        } else {
            1.0
        };
        let luxury_mult = (1.0 + savings_per_capita / 30.0).clamp(1.0, 8.0);

        for need in needs {
            let Some(res_market) = market.get(need.resource_id) else {
                continue;
            };

            let price = res_market.price;
            if price <= 0.0 {
                continue;
            }

            let wealth_mult = match need.category {
                crate::world::NeedCategory::LifeSupport => 1.0,
                crate::world::NeedCategory::Comfort => comfort_mult,
                crate::world::NeedCategory::Luxury => luxury_mult,
            };

            let physical_demand = need.amount_per_capita * wealth_mult * (subgroup.count as f64);
            let affordable_demand = budget_remaining / price;
            let effective_demand = physical_demand.min(affordable_demand);

            if effective_demand > 0.0 {
                budget_remaining -= effective_demand * price;
                let res_idx = need.resource_id.0 as usize;
                let national_demand = effective_demand * market_access;
                let local_demand = effective_demand - national_demand;

                if res_idx < demands_out.len() {
                    demands_out[res_idx] += national_demand;
                }
                if res_idx < local_demands_out.len() {
                    local_demands_out[res_idx] += local_demand;
                }
            }

            if budget_remaining <= 0.0 {
                break;
            }
        }
    }
}
