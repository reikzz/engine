use crate::economy::ResourceId;
use crate::system::{System, SystemContext};
use crate::world::CountryTag;
use crate::world::time::TimeTick;

#[derive(Debug, Clone, Copy)]
struct TradeExecution {
    exporter_tag: CountryTag,
    importer_tag: CountryTag,
    res_id: ResourceId,
    volume: f64,
    base_value: f64,
}

#[derive(Debug, Clone, Copy)]
struct MarketParticipant {
    tag: CountryTag,
    surplus: f64,
    deficit: f64,
    price_base: f64,
    tariff_rate: f64,
}

#[derive(Debug, Clone, Copy)]
struct FacImporter {
    index: usize,
    need: f64,
}

#[derive(Default)]
pub struct GlobalTradeSystem {
    warehouse_excess: Vec<Vec<f64>>,
    country_facilities: Vec<Vec<usize>>,
    country_tags: Vec<CountryTag>,
    facility_country_indices: Vec<usize>,
    planned_trades: Vec<TradeExecution>,
    participants: Vec<MarketParticipant>,
    fac_importers: Vec<FacImporter>,
    country_res_consumers: Vec<Vec<Vec<usize>>>,
    country_res_suppliers: Vec<Vec<Vec<usize>>>,
    country_res_stock: Vec<Vec<f64>>,
}

impl System for GlobalTradeSystem {
    fn id(&self) -> &'static str {
        "core:global_trade_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_day()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        ctx.global_market.reset_daily_stats();

        if ctx.countries.len() < 2 {
            return;
        }

        let SystemContext {
            defs,
            countries,
            facilities,
            companies,
            global_market,
            ..
        } = ctx;

        let num_countries = countries.len();
        let num_resources = defs.resources.len();

        self.country_tags.clear();
        for c in countries.iter() {
            self.country_tags.push(c.tag);
        }

        if self.country_facilities.len() != num_countries {
            self.country_facilities.resize_with(num_countries, Vec::new);
        }
        for sub_vec in &mut self.country_facilities {
            sub_vec.clear();
        }

        if self.country_res_consumers.len() != num_countries {
            self.country_res_consumers
                .resize_with(num_countries, || vec![Vec::new(); num_resources]);
        }
        for c_vec in &mut self.country_res_consumers {
            if c_vec.len() != num_resources {
                c_vec.resize_with(num_resources, Vec::new);
            }
            for res_vec in c_vec.iter_mut() {
                res_vec.clear();
            }
        }

        if self.country_res_suppliers.len() != num_countries {
            self.country_res_suppliers
                .resize_with(num_countries, || vec![Vec::new(); num_resources]);
        }
        for c_vec in &mut self.country_res_suppliers {
            if c_vec.len() != num_resources {
                c_vec.resize_with(num_resources, Vec::new);
            }
            for res_vec in c_vec.iter_mut() {
                res_vec.clear();
            }
        }

        if self.country_res_stock.len() != num_countries {
            self.country_res_stock
                .resize_with(num_countries, || vec![0.0; num_resources]);
        }
        for sub_vec in &mut self.country_res_stock {
            if sub_vec.len() != num_resources {
                sub_vec.resize(num_resources, 0.0);
            }
            sub_vec.fill(0.0);
        }

        if self.warehouse_excess.len() != num_countries {
            self.warehouse_excess
                .resize_with(num_countries, || vec![0.0; num_resources]);
        }
        for sub_vec in &mut self.warehouse_excess {
            if sub_vec.len() != num_resources {
                sub_vec.resize(num_resources, 0.0);
            }
            sub_vec.fill(0.0);
        }

        self.planned_trades.clear();

        if self.facility_country_indices.len() != facilities.len() {
            self.facility_country_indices
                .resize(facilities.len(), usize::MAX);
            for (f_idx, fac) in facilities.iter().enumerate() {
                if let Some(pos) = self.country_tags.iter().position(|t| *t == fac.country_tag) {
                    self.facility_country_indices[f_idx] = pos;
                }
            }
        }

        for (idx, fac) in facilities.iter().enumerate() {
            let c_idx = self.facility_country_indices[idx];
            if c_idx != usize::MAX {
                self.country_facilities[c_idx].push(idx);

                for (res_id, in_stock) in fac.stockpile.non_empty() {
                    let res_idx = res_id.0 as usize;
                    if res_idx < num_resources {
                        if in_stock > 0.0 {
                            self.country_res_suppliers[c_idx][res_idx].push(idx);
                            self.country_res_stock[c_idx][res_idx] += in_stock;
                        }
                        if in_stock > 500.0 {
                            self.warehouse_excess[c_idx][res_idx] += (in_stock - 500.0) * 0.05;
                        }
                    }
                }

                for line in &fac.lines {
                    if line.installed_capacity == 0 {
                        continue;
                    }
                    if let Some(recipe) = defs.recipes.get(line.recipe_id) {
                        for &(in_res, _) in &recipe.inputs {
                            let in_idx = in_res.0 as usize;
                            if in_idx < num_resources {
                                let consumers = &mut self.country_res_consumers[c_idx][in_idx];
                                if consumers.last().copied() != Some(idx) {
                                    consumers.push(idx);
                                }
                            }
                        }
                    }
                }
            }
        }

        for (res_id, _) in defs.resources.iter() {
            self.participants.clear();

            for (c_idx, country) in countries.iter().enumerate() {
                let Some(rm) = country.market.get(res_id) else {
                    continue;
                };
                let ex_rate = country.economy.exchange_rate.max(0.0001);
                let price_base = rm.price / ex_rate;

                let supplied = rm.last_supplied;
                let demanded = rm.last_demanded;

                let res_idx = res_id.0 as usize;
                let warehouse_excess = if res_idx < self.warehouse_excess[c_idx].len() {
                    self.warehouse_excess[c_idx][res_idx]
                } else {
                    0.0
                };

                let surplus = (supplied - demanded).max(0.0) + warehouse_excess;
                let deficit = (demanded - supplied).max(0.0);

                if surplus > 0.1 || deficit > 0.1 {
                    global_market.record_market_conditions(res_id, surplus, deficit);

                    self.participants.push(MarketParticipant {
                        tag: country.tag,
                        surplus,
                        deficit,
                        price_base,
                        tariff_rate: country.economy.tariff_rate,
                    });
                }
            }

            self.participants.sort_by(|a, b| {
                b.surplus
                    .partial_cmp(&a.surplus)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let exporters_count = self
                .participants
                .iter()
                .take_while(|p| p.surplus > 0.5)
                .count();

            for exp_idx in 0..exporters_count {
                let exp = self.participants[exp_idx];
                for imp_idx in 0..self.participants.len() {
                    let imp = self.participants[imp_idx];
                    if imp.deficit <= 0.5 || exp.tag == imp.tag {
                        continue;
                    }

                    let effective_import_price = exp.price_base * (1.0 + imp.tariff_rate);

                    if effective_import_price < imp.price_base * 1.05 {
                        let trade_volume = exp.surplus.min(imp.deficit) * 0.4;

                        if trade_volume > 0.1 {
                            let agreed_price_base = (exp.price_base + imp.price_base) * 0.5;
                            self.planned_trades.push(TradeExecution {
                                exporter_tag: exp.tag,
                                importer_tag: imp.tag,
                                res_id,
                                volume: trade_volume,
                                base_value: trade_volume * agreed_price_base,
                            });

                            self.participants[exp_idx].surplus -= trade_volume;
                            self.participants[imp_idx].deficit -= trade_volume;

                            if self.participants[exp_idx].surplus <= 0.5 {
                                break;
                            }
                        }
                    }
                }
            }
        }

        for trade in self.planned_trades.drain(..) {
            let exp_idx_opt = self
                .country_tags
                .iter()
                .position(|t| *t == trade.exporter_tag);
            let imp_idx_opt = self
                .country_tags
                .iter()
                .position(|t| *t == trade.importer_tag);

            let Some(exp_c_idx) = exp_idx_opt else {
                continue;
            };
            let Some(imp_c_idx) = imp_idx_opt else {
                continue;
            };

            let exp_ex_rate = countries[exp_c_idx].economy.exchange_rate.max(0.0001);
            let imp_ex_rate = countries[imp_c_idx].economy.exchange_rate.max(0.0001);
            let imp_tariff_rate = countries[imp_c_idx].economy.tariff_rate;

            let res_idx = trade.res_id.0 as usize;
            let available_stock_total: f64 = if res_idx < num_resources {
                self.country_res_stock[exp_c_idx][res_idx]
            } else {
                0.0
            };

            if available_stock_total <= 0.001 {
                continue;
            }

            let mut actual_volume = trade.volume.min(available_stock_total);
            let unit_price_base = trade.base_value / trade.volume.max(0.0001);
            let unit_cost_local = unit_price_base * imp_ex_rate;

            self.fac_importers.clear();
            let mut total_fac_need = 0.0;
            let mut total_fac_funds = 0.0;

            let consumer_indices = if res_idx < num_resources {
                &self.country_res_consumers[imp_c_idx][res_idx]
            } else {
                &self.country_facilities[imp_c_idx]
            };

            for &idx in consumer_indices {
                let fac = &facilities[idx];
                let mut fac_need = 0.0;
                for line in &fac.lines {
                    if line.installed_capacity == 0 {
                        continue;
                    }
                    if let Some(recipe) = defs.recipes.get(line.recipe_id) {
                        for &(in_res, in_amount) in &recipe.inputs {
                            if in_res == trade.res_id {
                                let target_stock =
                                    in_amount * (line.installed_capacity as f64) * 3.0;
                                let current_stock = fac.stockpile.get(in_res);
                                if current_stock < target_stock {
                                    fac_need += target_stock - current_stock;
                                }
                            }
                        }
                    }
                }

                if fac_need > 0.0 {
                    let comp_idx = fac.company_id.index();
                    let comp_bal = if comp_idx < companies.len() {
                        companies[comp_idx].balance.max(0.0)
                    } else {
                        0.0
                    };
                    let funds = comp_bal + fac.local_reserve.max(0.0);
                    if funds > 0.0 {
                        total_fac_need += fac_need;
                        total_fac_funds += funds;
                        self.fac_importers.push(FacImporter {
                            index: idx,
                            need: fac_need,
                        });
                    }
                }
            }

            let market_pool = countries[imp_c_idx]
                .market
                .get(trade.res_id)
                .map(|m| m.revenue_pool.max(0.0))
                .unwrap_or(0.0);

            let fac_max_vol = if unit_cost_local > 0.0 {
                total_fac_funds / (unit_cost_local * (1.0 + imp_tariff_rate))
            } else {
                0.0
            };
            let fac_vol_target = total_fac_need.min(fac_max_vol);
            let market_vol_target = if unit_cost_local > 0.0 {
                market_pool / (unit_cost_local * (1.0 + imp_tariff_rate))
            } else {
                0.0
            };
            let total_absorb_target = fac_vol_target + market_vol_target;

            if total_absorb_target <= 0.001 {
                continue;
            }

            actual_volume = actual_volume.min(total_absorb_target);

            let fac_volume = if total_absorb_target > 0.0 {
                actual_volume * (fac_vol_target / total_absorb_target)
            } else {
                0.0
            };
            let market_volume = actual_volume - fac_volume;

            let mut total_paid = 0.0;

            if fac_volume > 0.0 && total_fac_need > 0.0 {
                for imp in &self.fac_importers {
                    let fac = &mut facilities[imp.index];
                    let comp_idx = fac.company_id.index();
                    let share = (imp.need / total_fac_need) * fac_volume;
                    let gross_cost = share * unit_cost_local * (1.0 + imp_tariff_rate);
                    let avail = (if comp_idx < companies.len() {
                        companies[comp_idx].balance.max(0.0)
                    } else {
                        0.0
                    }) + fac.local_reserve.max(0.0);

                    if avail > 0.0 {
                        let actual_paid = gross_cost.min(avail);
                        let actual_received_share = if gross_cost > 0.0 {
                            share * (actual_paid / gross_cost)
                        } else {
                            0.0
                        };

                        let from_res = actual_paid.min(fac.local_reserve.max(0.0));
                        fac.local_reserve -= from_res;
                        if comp_idx < companies.len() {
                            let from_comp = (actual_paid - from_res).min(avail);
                            companies[comp_idx].balance -= from_comp;
                            companies[comp_idx].monthly_expenses += actual_paid;
                        }
                        fac.period_material_cost += actual_paid;
                        fac.stockpile.add(trade.res_id, actual_received_share);
                        total_paid += actual_paid;
                    }
                }
            }

            if market_volume > 0.0 {
                let gross_cost = market_volume * unit_cost_local * (1.0 + imp_tariff_rate);
                let actual_cost = gross_cost.min(market_pool);
                if let Some(rm) = countries[imp_c_idx].market.get_mut(trade.res_id) {
                    rm.revenue_pool -= actual_cost;
                }
                total_paid += actual_cost;
            }

            let tariff_total = total_paid * (imp_tariff_rate / (1.0 + imp_tariff_rate));
            if tariff_total > 0.0 {
                countries[imp_c_idx].economy.treasury += tariff_total;
                countries[imp_c_idx].economy.last_period_tariff_revenue += tariff_total;
            }

            global_market.record_trade(trade.res_id, actual_volume, unit_price_base);

            let export_revenue_local = actual_volume * unit_price_base * exp_ex_rate;
            let mut remaining_to_take = actual_volume;

            let supplier_indices = if res_idx < num_resources {
                &self.country_res_suppliers[exp_c_idx][res_idx]
            } else {
                &self.country_facilities[exp_c_idx]
            };

            for &idx in supplier_indices {
                if remaining_to_take <= 0.0 {
                    break;
                }
                let fac = &mut facilities[idx];
                let in_stock = fac.stockpile.get(trade.res_id);
                if in_stock > 0.0 {
                    let to_take = in_stock.min(remaining_to_take);
                    if fac.stockpile.try_consume(trade.res_id, to_take) {
                        remaining_to_take -= to_take;
                        if res_idx < num_resources {
                            self.country_res_stock[exp_c_idx][res_idx] =
                                (self.country_res_stock[exp_c_idx][res_idx] - to_take).max(0.0);
                        }
                        let share = (to_take / actual_volume) * export_revenue_local;
                        let comp_idx = fac.company_id.index();
                        if comp_idx < companies.len() {
                            companies[comp_idx].balance += share;
                            companies[comp_idx].monthly_revenue += share;
                        }
                        fac.period_revenue += share;
                    }
                }
            }
        }
    }
}
