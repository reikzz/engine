use crate::system::{System, SystemContext};
use crate::world::CountryTag;
use crate::world::time::TimeTick;
use std::collections::HashMap;

#[derive(Default)]
pub struct StructureSystem {
    tag_to_country_idx: HashMap<CountryTag, usize>,
}

impl System for StructureSystem {
    fn id(&self) -> &'static str {
        "core:structure_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_month()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        self.tag_to_country_idx.clear();
        for (idx, country) in ctx.countries.iter().enumerate() {
            self.tag_to_country_idx.insert(country.tag, idx);
        }

        for country in ctx.countries.iter_mut() {
            for s in country.structures.iter_mut() {
                s.total_demand = 0.0;
            }
        }

        for region in ctx.regions.iter_mut() {
            let Some(owner_tag) = region.owner else {
                continue;
            };
            let Some(&country_idx) = self.tag_to_country_idx.get(&owner_tag) else {
                continue;
            };
            let country = &mut ctx.countries[country_idx];

            for structure in region.structures.iter_mut() {
                let actual_spent = structure.period_wage_cost + structure.period_maintenance_cost;
                let unspent = structure.allocated_budget - actual_spent;

                if unspent > 0.0 {
                    country.economy.treasury += unspent;
                }

                structure.period_wage_cost = 0.0;
                structure.period_maintenance_cost = 0.0;

                let Some(def) = ctx.defs.structures.get(structure.def_id) else {
                    continue;
                };
                structure.calculate_demand(def, &ctx.defs.pop_types, &country.market);

                if let Some(state_struct) = country
                    .structures
                    .iter_mut()
                    .find(|s| s.kind == structure.kind)
                {
                    state_struct.total_demand += structure.required_budget;
                }
            }
        }

        for country in ctx.countries.iter_mut() {
            let total_country_planned: f64 = country
                .structures
                .iter()
                .map(|s| s.total_demand * s.funding_level)
                .sum();

            let tariffs = country.economy.last_period_tariff_revenue;
            country.economy.last_period_tariff_revenue = 0.0;
            let revenue = country.economy.last_period_tax_revenue + tariffs;

            let max_available = if revenue > 0.0 {
                let operational_cap = revenue * 0.85;
                let planned_budget = total_country_planned.min(operational_cap);
                planned_budget.min(country.economy.treasury.max(0.0))
            } else {
                total_country_planned.min(country.economy.treasury * 0.05)
            };

            let funding_scale =
                if total_country_planned > 0.0 && max_available < total_country_planned {
                    max_available / total_country_planned
                } else {
                    1.0
                };

            for state_struct in country.structures.iter_mut() {
                let planned =
                    state_struct.total_demand * state_struct.funding_level * funding_scale;
                let actual = planned.min(country.economy.treasury.max(0.0));

                country.economy.treasury -= actual;
                state_struct.monthly_budget = actual;
            }
        }

        for region in ctx.regions.iter_mut() {
            let Some(owner_tag) = region.owner else {
                continue;
            };
            let Some(&country_idx) = self.tag_to_country_idx.get(&owner_tag) else {
                continue;
            };
            let country = &ctx.countries[country_idx];

            for reg_struct in region.structures.iter_mut() {
                let Some(state_struct) = country
                    .structures
                    .iter()
                    .find(|s| s.kind == reg_struct.kind)
                else {
                    continue;
                };

                if state_struct.total_demand > 0.0 {
                    let share = reg_struct.required_budget / state_struct.total_demand;
                    let Some(def) = ctx.defs.structures.get(reg_struct.def_id) else {
                        continue;
                    };
                    let maintenance_needed =
                        def.base_maintenance_per_level * (reg_struct.level as f64);
                    let total_req: u32 =
                        def.workforce_requirements.iter().map(|(_, req)| *req).sum();
                    let workforce_ratio = if total_req == 0 {
                        1.0
                    } else {
                        let mut total_filled = 0;
                        for (pop_type, req) in &def.workforce_requirements {
                            let current = reg_struct
                                .current_workforce
                                .iter()
                                .find(|(t, _)| t == pop_type)
                                .map(|(_, c)| *c)
                                .unwrap_or(0);

                            total_filled += current.min(*req);
                        }
                        total_filled as f64 / total_req as f64
                    };

                    reg_struct.allocated_budget = state_struct.monthly_budget * share;
                    let maintenance_funded = maintenance_needed.min(reg_struct.allocated_budget);

                    let funding_ratio = if maintenance_needed > 0.0 {
                        maintenance_funded / maintenance_needed
                    } else {
                        1.0
                    };

                    if funding_ratio < 1.0 {
                        reg_struct.wear =
                            (reg_struct.wear + def.wear_per_month * (2.0 - funding_ratio)).min(1.0);
                    } else {
                        reg_struct.wear = (reg_struct.wear - def.wear_per_month * 0.5).max(0.0);
                    }

                    reg_struct.efficiency =
                        workforce_ratio * (1.0 - reg_struct.wear).max(0.0) * funding_ratio.min(1.0);
                } else {
                    reg_struct.allocated_budget = 0.0;
                    reg_struct.period_maintenance_cost = 0.0;
                }
            }
        }

        for region in ctx.regions.iter_mut() {
            region.modifiers = crate::world::map::RegionModifiers::default();
            for reg_struct in &region.structures {
                if reg_struct.efficiency <= 0.05 {
                    continue;
                }
                let Some(def) = ctx.defs.structures.get(reg_struct.def_id) else {
                    continue;
                };
                for (modifier, val) in &def.modifiers {
                    region.modifiers.add(
                        *modifier,
                        *val * (reg_struct.level as f64) * reg_struct.efficiency,
                    );
                }
            }
        }
    }
}
