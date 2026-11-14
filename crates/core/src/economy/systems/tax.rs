use crate::economy::Ownership;
use crate::system::{System, SystemContext};
use crate::world::CountryTag;
use crate::world::time::TimeTick;
use std::collections::HashMap;

#[derive(Debug, Default, Clone, Copy)]
pub struct TaxReport {
    pub corporate_tax_collected: f64,
    pub income_tax_collected: f64,
    pub tariff_collected: f64,
    pub total_tax_revenue: f64,
    pub corporate_dividends_transferred: f64,
}

#[derive(Default)]
pub struct TaxSystem {
    tag_to_country_idx: HashMap<CountryTag, usize>,
    region_id_to_idx: Vec<usize>,
    corporate_taxes: Vec<f64>,
    dividends: Vec<f64>,
    private_dividends: Vec<f64>,
    corporate_tax_rates: Vec<f64>,
    exchange_rates: Vec<f64>,
}

impl System for TaxSystem {
    fn id(&self) -> &'static str {
        "core:tax_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_month()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let pop_types = &ctx.defs.pop_types;
        let num_countries = ctx.countries.len();

        self.tag_to_country_idx.clear();
        for (idx, c) in ctx.countries.iter().enumerate() {
            self.tag_to_country_idx.insert(c.tag, idx);
        }

        self.corporate_taxes.clear();
        self.corporate_taxes.resize(num_countries, 0.0);

        self.dividends.clear();
        self.dividends.resize(num_countries, 0.0);

        self.private_dividends.clear();
        self.private_dividends.resize(num_countries, 0.0);

        self.corporate_tax_rates.clear();
        self.corporate_tax_rates.resize(num_countries, 0.0);

        self.exchange_rates.clear();
        self.exchange_rates.resize(num_countries, 1.0);

        for (idx, c) in ctx.countries.iter().enumerate() {
            self.corporate_tax_rates[idx] = c.economy.corporate_tax;
            self.exchange_rates[idx] = c.economy.exchange_rate.max(0.001);
        }

        let max_reg_id = ctx
            .regions
            .iter()
            .map(|r| r.id.0 as usize)
            .max()
            .unwrap_or(0);
        self.region_id_to_idx.clear();
        self.region_id_to_idx.resize(max_reg_id + 1, usize::MAX);
        for (idx, r) in ctx.regions.iter().enumerate() {
            self.region_id_to_idx[r.id.0 as usize] = idx;
        }

        for company in ctx.companies.iter_mut() {
            let profit = company.last_month_profit;
            let Some(&c_idx) = self.tag_to_country_idx.get(&company.country_tag) else {
                continue;
            };
            let corporate_tax_rate = self.corporate_tax_rates[c_idx];
            let exchange_rate = self.exchange_rates[c_idx];

            if profit > 0.0 {
                match &company.ownership {
                    Ownership::State => {
                        let divs = (profit * (1.0 - company.reinvestment_ratio))
                            .min(company.balance.max(0.0));
                        if divs > 0.0 {
                            company.balance -= divs;
                            self.dividends[c_idx] += divs;
                        }
                    }
                    Ownership::Private { .. } => {
                        let tax = (profit * corporate_tax_rate).min(company.balance.max(0.0));
                        if tax > 0.0 {
                            company.balance -= tax;
                            self.corporate_taxes[c_idx] += tax;
                        }

                        let net_profit = (profit - tax).max(0.0);
                        let divs = (net_profit * (1.0 - company.reinvestment_ratio))
                            .min(company.balance.max(0.0));
                        if divs > 0.0 {
                            company.balance -= divs;
                            self.private_dividends[c_idx] += divs;
                        }
                    }
                    Ownership::Regional(_) => {}
                }
            }

            let max_operational_reserve =
                (company.monthly_expenses * 24.0).max(5_000_000.0 * exchange_rate);
            if company.balance > max_operational_reserve {
                let excess = company.balance - max_operational_reserve;
                let sweep = excess * 0.10;
                company.balance -= sweep;
                match &company.ownership {
                    Ownership::State => {
                        self.dividends[c_idx] += sweep;
                    }
                    Ownership::Private { .. } => {
                        self.private_dividends[c_idx] += sweep;
                    }
                    Ownership::Regional(_) => {}
                }
            }
        }

        for (c_idx, country) in ctx.countries.iter_mut().enumerate() {
            let mut report = TaxReport::default();
            let income_tax_rate = country.economy.income_tax;
            let priv_divs = self.private_dividends[c_idx];

            if priv_divs > 0.0 {
                let mut total_shareholder_weight = 0.0;
                for region_id in &country.regions {
                    let reg_id = region_id.0 as usize;
                    if reg_id < self.region_id_to_idx.len() {
                        let r_idx = self.region_id_to_idx[reg_id];
                        if r_idx != usize::MAX && r_idx < ctx.regions.len() {
                            let region = &ctx.regions[r_idx];
                            for pop in &region.pops {
                                let total_count = pop.count();
                                if total_count > 0 {
                                    let weight = pop_types
                                        .get(pop.pop_type_id)
                                        .map(|t| t.political_weight as f64)
                                        .unwrap_or(1.0);
                                    total_shareholder_weight += weight * (total_count as f64);
                                }
                            }
                        }
                    }
                }

                if total_shareholder_weight > 0.0 {
                    for region_id in &country.regions {
                        let reg_id = region_id.0 as usize;
                        if reg_id < self.region_id_to_idx.len() {
                            let r_idx = self.region_id_to_idx[reg_id];
                            if r_idx != usize::MAX && r_idx < ctx.regions.len() {
                                let region = &mut ctx.regions[r_idx];
                                for pop in &mut region.pops {
                                    let total_count = pop.count();
                                    if total_count > 0 {
                                        let weight = pop_types
                                            .get(pop.pop_type_id)
                                            .map(|t| t.political_weight as f64)
                                            .unwrap_or(1.0);
                                        let pop_share = (weight * (total_count as f64))
                                            / total_shareholder_weight;
                                        let pop_div = priv_divs * pop_share;

                                        let emp_ratio =
                                            (pop.employed.count as f64) / (total_count as f64);
                                        let emp_div = pop_div * emp_ratio;
                                        let unemp_div = pop_div - emp_div;

                                        if pop.employed.count > 0 {
                                            pop.employed.savings += emp_div;
                                            pop.employed.period_income += emp_div;
                                        }
                                        if pop.unemployed.count > 0 {
                                            pop.unemployed.savings += unemp_div;
                                            pop.unemployed.period_income += unemp_div;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            for region_id in &country.regions {
                let reg_id = region_id.0 as usize;
                if reg_id >= self.region_id_to_idx.len() {
                    continue;
                }
                let r_idx = self.region_id_to_idx[reg_id];
                if r_idx == usize::MAX || r_idx >= ctx.regions.len() {
                    continue;
                }
                let region = &mut ctx.regions[r_idx];

                for pop in &mut region.pops {
                    if pop.employed.count == 0 || pop.employed.period_income <= 0.0 {
                        pop.employed.period_income = 0.0;
                        continue;
                    }

                    let is_taxable = pop_types
                        .get(pop.pop_type_id)
                        .map(|t| t.is_taxable)
                        .unwrap_or(false);

                    let taxable_base = pop.employed.period_income;

                    if is_taxable && taxable_base > 0.0 {
                        let tax = (taxable_base * income_tax_rate).min(pop.employed.savings);
                        pop.employed.savings -= tax;
                        report.income_tax_collected += tax;
                    }

                    pop.employed.period_income = 0.0;
                }
            }

            report.corporate_tax_collected = self.corporate_taxes[c_idx];
            report.corporate_dividends_transferred = self.dividends[c_idx];

            report.total_tax_revenue = report.corporate_tax_collected
                + report.income_tax_collected
                + report.tariff_collected
                + report.corporate_dividends_transferred;

            country.economy.last_period_tax_revenue = report.total_tax_revenue;
            country.economy.treasury += report.total_tax_revenue;
        }
    }
}
