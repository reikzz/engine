use crate::economy::market::NationalMarket;
use crate::system::{System, SystemContext};
use crate::world::time::TimeTick;
use crate::world::{CountryTag, PopNeed, PopSubgroup};

#[derive(Default)]
pub struct SettlementSystem {
    id_to_index: Vec<usize>,
    country_tags: Vec<CountryTag>,
    facility_country_indices: Vec<usize>,
    facility_region_indices: Vec<usize>,
    region_country_indices: Vec<usize>,
    region_id_to_index: Vec<usize>,
    cached_needs: Vec<Vec<PopNeed>>,
}

const BUFFER_DAYS: f64 = 3.0;

impl System for SettlementSystem {
    fn id(&self) -> &'static str {
        "core:settlement_system"
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
        self.id_to_index.clear();
        self.id_to_index.resize(max_id + 1, usize::MAX);
        for (idx, comp) in ctx.companies.iter().enumerate() {
            self.id_to_index[comp.id.0 as usize] = idx;
        }

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

        self.facility_region_indices.clear();
        self.facility_region_indices
            .resize(ctx.facilities.len(), usize::MAX);
        for (f_idx, fac) in ctx.facilities.iter().enumerate() {
            let r_id = fac.region_id.0 as usize;
            if r_id < self.region_id_to_index.len() {
                self.facility_region_indices[f_idx] = self.region_id_to_index[r_id];
            }
        }

        self.facility_country_indices.clear();
        self.facility_country_indices
            .resize(ctx.facilities.len(), usize::MAX);
        for (f_idx, fac) in ctx.facilities.iter().enumerate() {
            if let Some(pos) = self.country_tags.iter().position(|t| *t == fac.country_tag) {
                self.facility_country_indices[f_idx] = pos;
            }
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

        let SystemContext {
            defs,
            facilities,
            companies,
            countries,
            regions,
            ..
        } = ctx;

        let recipes = &defs.recipes;

        for (r_idx, region) in regions.iter_mut().enumerate() {
            let country_idx = self.region_country_indices[r_idx];
            if country_idx == usize::MAX {
                continue;
            }
            let market = &mut countries[country_idx].market;
            let market_access = region.market_access;
            let local_prices = region.local_prices.as_slice();
            let local_buy_fulfillment = region.local_buy_fulfillment.as_slice();

            for pop in &mut region.pops {
                let pop_type_idx = pop.pop_type_id.0 as usize;
                let pop_needs = if pop_type_idx < self.cached_needs.len() {
                    self.cached_needs[pop_type_idx].as_slice()
                } else {
                    &[]
                };

                if pop_needs.is_empty() {
                    pop.satisfaction = 1.0;
                    continue;
                }

                Self::consume_needs(
                    &mut pop.employed,
                    pop_needs,
                    market,
                    market_access,
                    local_prices,
                    local_buy_fulfillment,
                );
                Self::consume_needs(
                    &mut pop.unemployed,
                    pop_needs,
                    market,
                    market_access,
                    local_prices,
                    local_buy_fulfillment,
                );

                let total_count = pop.count();
                if total_count > 0 {
                    pop.satisfaction = ((pop.employed.needs_met * pop.employed.count as f32)
                        + (pop.unemployed.needs_met * pop.unemployed.count as f32))
                        / (total_count as f32);
                } else {
                    pop.satisfaction = 1.0;
                }
            }
        }

        for (fac_idx, facility) in facilities.iter_mut().enumerate() {
            let country_idx = self.facility_country_indices[fac_idx];
            if country_idx == usize::MAX {
                continue;
            }
            let market = &mut countries[country_idx].market;
            let reg_idx = self.facility_region_indices[fac_idx];
            let reg = if reg_idx != usize::MAX && reg_idx < regions.len() {
                Some(&regions[reg_idx])
            } else {
                None
            };
            let mut facility_day_revenue = 0.0;

            for line in &facility.lines {
                if line.installed_capacity == 0 {
                    continue;
                }

                let Some(recipe) = recipes.get(line.recipe_id) else {
                    continue;
                };

                for &(res_id, _) in &recipe.outputs {
                    let Some(res_market) = market.get_mut(res_id) else {
                        continue;
                    };

                    let res_idx = res_id.0 as usize;
                    let (effective_price, effective_sell_fulfillment) = if let Some(r) = reg {
                        (
                            r.effective_price(res_idx, res_market.price),
                            r.effective_sell_fulfillment(res_idx, res_market.sell_fulfillment),
                        )
                    } else {
                        (res_market.price, res_market.sell_fulfillment)
                    };

                    if effective_sell_fulfillment <= 0.0 || effective_price <= 0.0 {
                        continue;
                    }

                    let available_stock = facility.stockpile.get(res_id);
                    if available_stock <= 0.0 {
                        continue;
                    }

                    let desired_qty = available_stock * effective_sell_fulfillment;
                    let expected_revenue = desired_qty * effective_price;

                    let payout = expected_revenue.min(res_market.revenue_pool);
                    let actual_sold_qty = payout / effective_price;

                    if actual_sold_qty > 0.0
                        && facility.stockpile.try_consume(res_id, actual_sold_qty)
                    {
                        res_market.revenue_pool -= payout;
                        facility_day_revenue += payout;
                        facility.period_revenue += payout;
                    }
                }
            }

            let comp_id = facility.company_id.0 as usize;
            if facility_day_revenue > 0.0 && comp_id < self.id_to_index.len() {
                let idx = self.id_to_index[comp_id];
                if idx != usize::MAX && idx < companies.len() {
                    let reserve_target = 25_000.0;
                    if facility.local_reserve < reserve_target {
                        let to_res =
                            facility_day_revenue.min(reserve_target - facility.local_reserve);
                        facility.local_reserve += to_res;
                        companies[idx].balance += facility_day_revenue - to_res;
                    } else {
                        companies[idx].balance += facility_day_revenue;
                    }
                }
            }
        }

        for (fac_idx, facility) in facilities.iter_mut().enumerate() {
            let comp_id = facility.company_id.0 as usize;
            let comp_idx = if comp_id < self.id_to_index.len() {
                self.id_to_index[comp_id]
            } else {
                usize::MAX
            };
            if comp_idx == usize::MAX || comp_idx >= companies.len() {
                continue;
            }
            let company = &mut companies[comp_idx];
            let total_available = company.balance.max(0.0) + facility.local_reserve.max(0.0);
            if total_available <= 0.0 {
                continue;
            }

            let country_idx = self.facility_country_indices[fac_idx];
            if country_idx == usize::MAX {
                continue;
            }
            let market = &mut countries[country_idx].market;
            let reg_idx = self.facility_region_indices[fac_idx];
            let reg = if reg_idx != usize::MAX && reg_idx < regions.len() {
                Some(&regions[reg_idx])
            } else {
                None
            };

            for line in &facility.lines {
                if line.installed_capacity == 0 {
                    continue;
                }

                let Some(recipe) = recipes.get(line.recipe_id) else {
                    continue;
                };
                let cap = line.installed_capacity as f64;

                for &(res_id, req_per_cap) in &recipe.inputs {
                    let total_available =
                        company.balance.max(0.0) + facility.local_reserve.max(0.0);
                    if total_available <= 0.0 {
                        break;
                    }

                    let is_hydrocarbon = defs
                        .resources
                        .get(res_id)
                        .map(|r| r.key == "crude_oil" || r.key == "natural_gas")
                        .unwrap_or(false);

                    let supplier_region = countries[country_idx]
                        .regions
                        .first()
                        .copied()
                        .unwrap_or(facility.region_id);

                    let route = ctx.transport_network.find_route(
                        facility.region_id,
                        supplier_region,
                        is_hydrocarbon,
                    );

                    let dynamic_buffer =
                        (BUFFER_DAYS as f64).max(route.lead_time_days as f64 + 1.0);
                    let target_stock = req_per_cap * cap * dynamic_buffer;
                    let current_stock = facility.stockpile.get(res_id);
                    let incoming = facility.pending_amount(res_id);
                    let effective_stock = current_stock + incoming;

                    if effective_stock >= target_stock {
                        continue;
                    }

                    let missing = target_stock - effective_stock;
                    let Some(res_market) = market.get_mut(res_id) else {
                        continue;
                    };

                    let res_idx = res_id.0 as usize;
                    let (effective_price, effective_buy_fulfillment) = if let Some(r) = reg {
                        (
                            r.effective_price(res_idx, res_market.price),
                            r.effective_buy_fulfillment(res_idx, res_market.buy_fulfillment),
                        )
                    } else {
                        (res_market.price, res_market.buy_fulfillment)
                    };

                    let market_available = missing * effective_buy_fulfillment;
                    if market_available <= 0.0 {
                        continue;
                    }

                    let base_cost = market_available * effective_price;
                    let freight_rate =
                        (route.distance_km * 0.00015 * route.fuel_cost_factor).min(0.60);
                    let unit_price_with_freight = effective_price * (1.0 + freight_rate);
                    let total_cost = market_available * unit_price_with_freight;

                    let (bought, cost, freight_paid) = if total_available >= total_cost {
                        (market_available, base_cost, base_cost * freight_rate)
                    } else {
                        let units = total_available / unit_price_with_freight;
                        let b_cost = units * effective_price;
                        (units, b_cost, total_available - b_cost)
                    };

                    let total_spent = cost + freight_paid;
                    if bought > 0.0 && total_available >= total_spent {
                        if route.lead_time_days == 0 {
                            facility.stockpile.add(res_id, bought);
                        } else {
                            facility
                                .pending_deliveries
                                .push(crate::economy::PendingDelivery {
                                    resource_id: res_id,
                                    amount: bought,
                                    arrival_date: ctx.date.add_days(route.lead_time_days),
                                });
                        }

                        let from_reserve = total_spent.min(facility.local_reserve.max(0.0));
                        facility.local_reserve -= from_reserve;
                        let from_comp = (total_spent - from_reserve).min(company.balance.max(0.0));
                        company.balance -= from_comp;
                        facility.period_material_cost += total_spent;
                        res_market.revenue_pool += cost;

                        let fuel_burn =
                            bought * route.distance_km * 0.000005 * route.fuel_cost_factor;
                        if fuel_burn > 0.0 {
                            if let Some(fuel_id) = defs.resources.get_id("fuel") {
                                market.record_demand(fuel_id, fuel_burn);
                            }
                        }
                    }
                }
            }
        }

        for (r_idx, region) in regions.iter_mut().enumerate() {
            let country_idx = self.region_country_indices[r_idx];
            if country_idx == usize::MAX {
                continue;
            }
            let market = &mut countries[country_idx].market;

            for structure in region.structures.iter_mut() {
                if structure.allocated_budget <= 0.0 {
                    continue;
                }

                let Some(def) = defs.structures.get(structure.def_id) else {
                    continue;
                };

                let remaining_budget =
                    (structure.allocated_budget - structure.period_maintenance_cost).max(0.0);
                if remaining_budget <= 0.0 {
                    continue;
                }

                for &(res_id, monthly_amount) in &def.maintenance_goods {
                    let Some(res_market) = market.get_mut(res_id) else {
                        continue;
                    };
                    if res_market.price <= 0.0 || res_market.buy_fulfillment <= 0.0 {
                        continue;
                    }

                    let daily_needed = (monthly_amount * structure.level as f64) / 30.0;
                    let available_qty = daily_needed * res_market.buy_fulfillment;
                    let cost = available_qty * res_market.price;

                    let (_, actual_cost) = if remaining_budget >= cost {
                        (available_qty, cost)
                    } else {
                        (remaining_budget / res_market.price, remaining_budget)
                    };

                    if actual_cost > 0.0 {
                        structure.period_maintenance_cost += actual_cost;
                        res_market.revenue_pool += actual_cost;
                    }
                }
            }
        }
    }
}

impl SettlementSystem {
    #[inline]
    fn consume_needs(
        subgroup: &mut PopSubgroup,
        needs: &[PopNeed],
        market: &mut NationalMarket,
        market_access: f64,
        local_prices: &[f64],
        local_buy_fulfillment: &[f64],
    ) {
        if subgroup.count == 0 {
            return;
        }

        if subgroup.savings <= 0.0 {
            subgroup.needs_met = (subgroup.needs_met * 0.9).max(0.0);
            return;
        }

        let mut total_weight = 0.0;
        let mut total_fulfillment = 0.0;
        let savings_per_capita = (subgroup.savings / (subgroup.count as f64).max(1.0)).max(0.0);

        let comfort_mult = if savings_per_capita > 50.0 {
            (1.0 + (savings_per_capita / 50.0).log2()).clamp(1.0, 4.0)
        } else {
            1.0
        };
        let luxury_mult = (1.0 + savings_per_capita / 30.0).clamp(1.0, 8.0);

        for need in needs {
            let weight = if need.weight > 0.0 { need.weight } else { 1.0 };
            total_weight += weight;

            let base_physical_demand = need.amount_per_capita * (subgroup.count as f64);
            if base_physical_demand <= 0.0 {
                total_fulfillment += weight;
                continue;
            }

            let wealth_mult = match need.category {
                crate::world::NeedCategory::LifeSupport => 1.0,
                crate::world::NeedCategory::Comfort => comfort_mult,
                crate::world::NeedCategory::Luxury => luxury_mult,
            };

            let physical_demand = base_physical_demand * wealth_mult;
            let Some(res_market) = market.get_mut(need.resource_id) else {
                continue;
            };

            let national_price = res_market.price;
            let res_idx = need.resource_id.0 as usize;
            let local_price = local_prices.get(res_idx).copied().unwrap_or(0.0);
            let effective_price = if local_price > 0.0 {
                (market_access * national_price + (1.0 - market_access) * local_price).max(0.01)
            } else {
                national_price
            };

            if effective_price <= 0.0 {
                continue;
            }

            let national_fulfillment = res_market.buy_fulfillment;
            let local_fulfillment = local_buy_fulfillment.get(res_idx).copied().unwrap_or(1.0);
            let effective_fulfillment = (market_access * national_fulfillment
                + (1.0 - market_access) * local_fulfillment)
                .clamp(0.0, 1.0);

            let desired_expenditure = physical_demand * effective_price;
            let affordable_expenditure = desired_expenditure.min(subgroup.savings);
            let units_desired = affordable_expenditure / effective_price;

            let units_purchased = units_desired * effective_fulfillment;
            let actual_cost = units_purchased * effective_price;

            if actual_cost > 0.0 {
                subgroup.savings = (subgroup.savings - actual_cost).max(0.0);
                res_market.revenue_pool += actual_cost;
            }

            let fulfillment_ratio = if physical_demand > 0.0 {
                (units_purchased / physical_demand).clamp(0.0, 1.0)
            } else {
                1.0
            };

            total_fulfillment += weight * fulfillment_ratio;
        }

        let current_day_met = if total_weight > 0.0 {
            (total_fulfillment / total_weight).clamp(0.0, 1.0) as f32
        } else {
            1.0
        };

        subgroup.needs_met = subgroup.needs_met * 0.8 + current_day_met * 0.2;
    }
}
