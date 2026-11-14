use crate::economy::ResourceId;
use crate::system::{System, SystemContext};
use crate::world::CountryTag;
use crate::world::time::TimeTick;

#[derive(Default)]
pub struct MarketDistribution {
    country_tags: Vec<CountryTag>,
    country_demands: Vec<Vec<f64>>,
    country_supplies: Vec<Vec<f64>>,
    daily_consumption: Vec<(ResourceId, f64)>,
    region_id_to_index: Vec<usize>,
}

const BUFFER_DAYS: f64 = 3.0;

impl System for MarketDistribution {
    fn id(&self) -> &'static str {
        "core:market_distribution_system"
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

        let max_reg_id = ctx
            .regions
            .iter()
            .map(|r| r.id.0 as usize)
            .max()
            .unwrap_or(0);
        self.region_id_to_index.clear();
        self.region_id_to_index.resize(max_reg_id + 1, usize::MAX);
        for (idx, r) in ctx.regions.iter().enumerate() {
            self.region_id_to_index[r.id.0 as usize] = idx;
        }

        if self.country_demands.len() != num_countries {
            self.country_demands
                .resize_with(num_countries, || vec![0.0; num_resources]);
        }
        if self.country_supplies.len() != num_countries {
            self.country_supplies
                .resize_with(num_countries, || vec![0.0; num_resources]);
        }

        for demands in &mut self.country_demands {
            if demands.len() != num_resources {
                demands.resize(num_resources, 0.0);
            }
            demands.fill(0.0);
        }
        for supplies in &mut self.country_supplies {
            if supplies.len() != num_resources {
                supplies.resize(num_resources, 0.0);
            }
            supplies.fill(0.0);
        }

        for facility in ctx.facilities.iter() {
            let Some(c_idx) = self
                .country_tags
                .iter()
                .position(|t| *t == facility.country_tag)
            else {
                continue;
            };

            let reg_pos = if (facility.region_id.0 as usize) < self.region_id_to_index.len() {
                self.region_id_to_index[facility.region_id.0 as usize]
            } else {
                usize::MAX
            };
            let market_access = if reg_pos != usize::MAX {
                ctx.regions[reg_pos].market_access
            } else {
                1.0
            };

            self.daily_consumption.clear();

            for line in &facility.lines {
                if let Some(recipe) = ctx.defs.recipes.get(line.recipe_id) {
                    for &(res_id, _) in &recipe.outputs {
                        let available_to_sell = facility.stockpile.get(res_id);
                        if available_to_sell > 0.0 {
                            let res_idx = res_id.0 as usize;
                            if res_idx < num_resources {
                                let national_amount = available_to_sell * market_access;
                                let local_amount = available_to_sell - national_amount;
                                self.country_supplies[c_idx][res_idx] += national_amount;
                                if reg_pos != usize::MAX
                                    && res_idx < ctx.regions[reg_pos].local_supplied.len()
                                {
                                    ctx.regions[reg_pos].local_supplied[res_idx] += local_amount;
                                }
                            }
                        }
                    }
                }

                if line.installed_capacity == 0 {
                    continue;
                }

                if let Some(recipe) = &ctx.defs.recipes.get(line.recipe_id) {
                    let cap = line.installed_capacity as f64;
                    for &(res_id, req_per_cap) in &recipe.inputs {
                        let burn = req_per_cap * cap;
                        if let Some(entry) = self
                            .daily_consumption
                            .iter_mut()
                            .find(|(id, _)| *id == res_id)
                        {
                            entry.1 += burn;
                        } else {
                            self.daily_consumption.push((res_id, burn));
                        }
                    }
                }
            }

            let supplier_region = if c_idx < ctx.countries.len() {
                ctx.countries[c_idx]
                    .regions
                    .first()
                    .copied()
                    .unwrap_or(facility.region_id)
            } else {
                facility.region_id
            };

            for &(res_id, daily_burn) in &self.daily_consumption {
                let is_hydrocarbon = ctx
                    .defs
                    .resources
                    .get(res_id)
                    .map(|r| r.key == "crude_oil" || r.key == "natural_gas")
                    .unwrap_or(false);
                let route = ctx.transport_network.find_route(
                    facility.region_id,
                    supplier_region,
                    is_hydrocarbon,
                );
                let dynamic_buffer = (BUFFER_DAYS as f64).max(route.lead_time_days as f64 + 1.0);
                let target_stock = daily_burn * dynamic_buffer;
                let current_stock = facility.stockpile.get(res_id);
                let incoming = facility.pending_amount(res_id);
                let effective_stock = current_stock + incoming;

                if effective_stock < target_stock {
                    let needed = target_stock - effective_stock;
                    let res_idx = res_id.0 as usize;
                    if res_idx < num_resources {
                        let national_needed = needed * market_access;
                        let local_needed = needed - national_needed;
                        self.country_demands[c_idx][res_idx] += national_needed;
                        if reg_pos != usize::MAX
                            && res_idx < ctx.regions[reg_pos].local_demanded.len()
                        {
                            ctx.regions[reg_pos].local_demanded[res_idx] += local_needed;
                        }
                    }
                }
            }
        }

        for region in ctx.regions.iter() {
            let Some(owner) = region.owner else { continue };
            let Some(c_idx) = self.country_tags.iter().position(|t| *t == owner) else {
                continue;
            };

            for structure in region.structures.iter() {
                if structure.allocated_budget <= 0.0 {
                    continue;
                };

                let Some(def) = &ctx.defs.structures.get(structure.def_id) else {
                    continue;
                };

                for &(res_id, amount) in &def.maintenance_goods {
                    let total_amount = amount * (structure.level as f64) / 30.0;

                    if total_amount > 0.0 {
                        let res_idx = res_id.0 as usize;
                        if res_idx < num_resources {
                            self.country_demands[c_idx][res_idx] += total_amount;
                        }
                    }
                }
            }
        }

        for (c_idx, country) in ctx.countries.iter().enumerate() {
            for project in &country.construction_queue {
                for &(res_id, daily_needed) in &project.daily_materials {
                    if daily_needed > 0.0 {
                        let res_idx = res_id.0 as usize;
                        if res_idx < num_resources {
                            self.country_demands[c_idx][res_idx] += daily_needed;
                        }
                    }
                }
            }
        }

        for (c_idx, country) in ctx.countries.iter_mut().enumerate() {
            let demands = &self.country_demands[c_idx];
            let supplies = &self.country_supplies[c_idx];

            for (res_id, _) in ctx.defs.resources.iter() {
                let res_idx = res_id.0 as usize;
                if res_idx < num_resources {
                    let d = demands[res_idx];
                    if d > 0.0 {
                        country.market.record_demand(res_id, d);
                    }
                    let s = supplies[res_idx];
                    if s > 0.0 {
                        country.market.record_supply(res_id, s);
                    }
                }
            }
        }
    }
}
