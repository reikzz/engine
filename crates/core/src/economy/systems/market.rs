use crate::system::{System, SystemContext};
use crate::world::CountryTag;
use crate::world::time::TimeTick;

#[derive(Default)]
pub struct MarketSystem {
    base_prices: Vec<f64>,
    country_tags: Vec<CountryTag>,
    national_prices: Vec<Vec<f64>>,
}

impl System for MarketSystem {
    fn id(&self) -> &'static str {
        "core:market_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_day()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        for country in ctx.countries.iter_mut() {
            country.market.update_prices();
        }

        let num_resources = ctx.defs.resources.len();
        if self.base_prices.len() != num_resources {
            self.base_prices.clear();
            for (_, res) in ctx.defs.resources.iter() {
                self.base_prices.push(res.base_price);
            }
        }

        self.country_tags.clear();
        for country in ctx.countries.iter() {
            self.country_tags.push(country.tag);
        }

        if self.national_prices.len() != ctx.countries.len() {
            self.national_prices
                .resize_with(ctx.countries.len(), || vec![0.0; num_resources]);
        }

        for (c_idx, country) in ctx.countries.iter().enumerate() {
            let prices = &mut self.national_prices[c_idx];
            if prices.len() != num_resources {
                prices.resize(num_resources, 0.0);
            }
            for (res_id, _) in ctx.defs.resources.iter() {
                let r_idx = res_id.0 as usize;
                if r_idx < prices.len() {
                    prices[r_idx] = country.market.price(res_id);
                }
            }
        }

        for region in ctx.regions.iter_mut() {
            let Some(owner) = region.owner else { continue };
            let Some(c_idx) = self.country_tags.iter().position(|t| *t == owner) else {
                continue;
            };
            let prices = &self.national_prices[c_idx];
            region.update_local_market(&self.base_prices, prices);
        }
    }
}
