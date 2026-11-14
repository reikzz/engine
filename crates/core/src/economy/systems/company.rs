use crate::system::{System, SystemContext};
use crate::world::time::TimeTick;
use std::collections::HashMap;

#[derive(Default)]
pub struct CompanySystem {
    id_to_index: Vec<usize>,
    tag_to_country_idx: HashMap<crate::CountryTag, usize>,
    country_value_added: Vec<f64>,
}

impl System for CompanySystem {
    fn id(&self) -> &'static str {
        "core:company_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_month()
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

        self.tag_to_country_idx.clear();
        for (idx, country) in ctx.countries.iter().enumerate() {
            self.tag_to_country_idx.insert(country.tag, idx);
        }

        self.country_value_added.clear();
        self.country_value_added.resize(ctx.countries.len(), 0.0);

        for facility in ctx.facilities.iter_mut() {
            let comp_id = facility.company_id.0 as usize;
            if comp_id < self.id_to_index.len() {
                let comp_idx = self.id_to_index[comp_id];
                if comp_idx != usize::MAX && comp_idx < ctx.companies.len() {
                    ctx.companies[comp_idx].monthly_revenue += facility.period_revenue;
                    ctx.companies[comp_idx].monthly_expenses +=
                        facility.period_material_cost + facility.period_wage_cost;
                }
            }

            let value_added = (facility.period_revenue - facility.period_material_cost).max(0.0);
            if let Some(&country_idx) = self.tag_to_country_idx.get(&facility.country_tag) {
                self.country_value_added[country_idx] += value_added;
            }

            facility.reset_period_finances();
        }

        for company in ctx.companies.iter_mut() {
            company.last_month_profit = company.monthly_revenue - company.monthly_expenses;
            company.monthly_revenue = 0.0;
            company.monthly_expenses = 0.0;
        }

        for region in ctx.regions.iter() {
            if let Some(owner) = region.owner {
                let struct_val: f64 = region
                    .structures
                    .iter()
                    .map(|s| s.allocated_budget.max(s.required_budget * 0.5))
                    .sum();
                if let Some(&country_idx) = self.tag_to_country_idx.get(&owner) {
                    self.country_value_added[country_idx] += struct_val;
                }
            }
        }

        for (idx, country) in ctx.countries.iter_mut().enumerate() {
            let monthly_gdp = self.country_value_added[idx];
            let annualized_gdp = monthly_gdp * 12.0;

            if annualized_gdp > 0.0 {
                if country.economy.nominal_gdp <= 0.0 {
                    country.economy.nominal_gdp = annualized_gdp;
                } else {
                    country.economy.nominal_gdp =
                        country.economy.nominal_gdp * 0.3 + annualized_gdp * 0.7;
                }
            }
        }
    }
}
