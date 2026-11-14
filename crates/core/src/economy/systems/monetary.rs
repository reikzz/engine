use crate::system::{System, SystemContext};
use crate::world::time::TimeTick;

#[derive(Default)]
pub struct MonetarySystem;

impl System for MonetarySystem {
    fn id(&self) -> &'static str {
        "core:monetary_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_day()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let is_monthly_sample = ctx.tick.is_new_month();

        for country in ctx.countries.iter_mut() {
            if country.economy.national_debt > 0.0 {
                let daily_debt_service =
                    (country.economy.national_debt * country.economy.central_bank_rate / 365.0)
                        .max(0.0);
                country.economy.treasury -= daily_debt_service;
                country.economy.last_period_expenses += daily_debt_service;
            }

            let mut total_weighted_price_ratio = 0.0f64;
            let mut total_weight = 0.0f64;

            for region in ctx.regions.iter().filter(|r| r.owner == Some(country.tag)) {
                for pop in &region.pops {
                    let pop_count = (pop.employed.count + pop.unemployed.count) as f64;
                    if pop_count <= 0.0 {
                        continue;
                    }

                    if let Some(pop_def) = ctx.defs.pop_types.get(pop.pop_type_id) {
                        for need in &pop_def.needs {
                            let res_id = need.resource_id;
                            let base_price = ctx
                                .defs
                                .resources
                                .get(res_id)
                                .map(|r| r.base_price)
                                .unwrap_or(10.0)
                                .max(0.01);

                            let local_price = region
                                .local_prices
                                .get(res_id.0 as usize)
                                .copied()
                                .unwrap_or_else(|| {
                                    country
                                        .market
                                        .get(res_id)
                                        .map(|m| m.price)
                                        .unwrap_or(base_price)
                                });

                            let weight = pop_count * need.amount_per_capita.max(0.0) * base_price;
                            total_weighted_price_ratio += weight * (local_price / base_price);
                            total_weight += weight;
                        }
                    }
                }
            }

            if total_weight > 0.0 {
                country.economy.cpi =
                    (100.0 * total_weighted_price_ratio / total_weight).clamp(10.0, 1000.0);
            }

            if is_monthly_sample {
                let current_cpi = country.economy.cpi;
                let cursor = country.economy.cpi_history_cursor as usize;
                let oldest_cpi = if country.economy.cpi_history_len >= 12 {
                    country.economy.cpi_monthly_history[cursor]
                } else {
                    country.economy.cpi_monthly_history[0]
                };

                country.economy.cpi_monthly_history[cursor] = current_cpi;
                country.economy.cpi_history_cursor = ((cursor + 1) % 12) as u8;

                if country.economy.cpi_history_len < 12 {
                    country.economy.cpi_history_len += 1;
                    let months_elapsed = country.economy.cpi_history_len as f64;
                    let accumulated_change = (current_cpi - 100.0) / 100.0;
                    let annualized = accumulated_change * (12.0 / months_elapsed.max(1.0));
                    country.economy.inflation_rate = annualized.clamp(-0.10, 0.50);
                } else {
                    let annual_inflation = (current_cpi - oldest_cpi) / oldest_cpi.max(1.0);
                    country.economy.inflation_rate = annual_inflation.clamp(-0.10, 0.50);
                }

                if country.economy.inflation_rate > 0.04 {
                    let excess_inflation = country.economy.inflation_rate - 0.04;
                    let radicalism_delta = (0.05 * excess_inflation) as f32;
                    let satisfaction_delta = (0.05 * excess_inflation) as f32;

                    for region in ctx
                        .regions
                        .iter_mut()
                        .filter(|r| r.owner == Some(country.tag))
                    {
                        for pop in &mut region.pops {
                            pop.radicalism = (pop.radicalism + radicalism_delta).clamp(0.0, 1.0);
                            pop.satisfaction =
                                (pop.satisfaction - satisfaction_delta).clamp(0.0, 1.0);
                        }
                    }
                }
            }
        }
    }
}
