use crate::Country;
use crate::economy::finance::BankLoan;
use crate::economy::{CompanyState, FacilityState, Ownership, SectorKind};
use crate::system::{System, SystemContext};
use crate::world::BailoutPolicy;
use crate::world::CountryTag;
use crate::world::map::Region;
use crate::world::time::TimeTick;
use std::collections::HashMap;

#[derive(Default)]
pub struct BankingSystem {
    tag_to_country_idx: HashMap<CountryTag, usize>,
    country_exchange_rates: Vec<f64>,
    country_regions: Vec<Vec<usize>>,
    borrower_active_facilities: Vec<bool>,
    company_has_facilities: Vec<bool>,
    active_loan_counts: Vec<usize>,
    commercial_banks: Vec<Option<usize>>,
    consumer_banks: Vec<Option<usize>>,
}

impl System for BankingSystem {
    fn id(&self) -> &'static str {
        "core:banking_system"
    }

    fn should_run(&self, tick: &TimeTick) -> bool {
        tick.is_new_month()
    }

    fn update(&mut self, ctx: &mut SystemContext) {
        let SystemContext {
            countries,
            regions,
            facilities,
            companies,
            loans,
            ..
        } = ctx;

        let num_countries = countries.len();

        self.tag_to_country_idx.clear();
        for (idx, country) in countries.iter().enumerate() {
            self.tag_to_country_idx.insert(country.tag, idx);
        }

        self.country_exchange_rates.clear();
        self.country_exchange_rates.resize(num_countries, 1.0);
        for (idx, country) in countries.iter().enumerate() {
            self.country_exchange_rates[idx] = country.economy.exchange_rate.max(0.001);
        }

        if self.country_regions.len() < num_countries {
            self.country_regions.resize(num_countries, Vec::new());
        }
        for sub_vec in &mut self.country_regions[..num_countries] {
            sub_vec.clear();
        }
        for (r_idx, region) in regions.iter().enumerate() {
            if let Some(owner) = region.owner {
                if let Some(&c_idx) = self.tag_to_country_idx.get(&owner) {
                    self.country_regions[c_idx].push(r_idx);
                }
            }
        }

        let max_comp_id = companies.iter().map(|c| c.id.0 as usize).max().unwrap_or(0);

        self.borrower_active_facilities.clear();
        self.borrower_active_facilities
            .resize(max_comp_id + 1, false);

        self.company_has_facilities.clear();
        self.company_has_facilities.resize(max_comp_id + 1, false);

        for fac in facilities.iter() {
            let comp_id = fac.company_id.0 as usize;
            if comp_id <= max_comp_id {
                self.company_has_facilities[comp_id] = true;
                if fac.lines.iter().any(|l| l.installed_capacity > 0) {
                    self.borrower_active_facilities[comp_id] = true;
                }
            }
        }

        self.process_loan_repayments(companies, facilities, loans);
        self.collect_state_bank_dividends(countries, companies);
        self.distribute_deposit_interest(countries, regions, companies);
        self.process_consumer_banking(countries, regions, companies);
        self.process_working_capital_loans(companies, loans);
        self.process_state_recapitalization(companies, countries);
    }
}

impl BankingSystem {
    fn process_loan_repayments(
        &self,
        companies: &mut [CompanyState],
        facilities: &mut [FacilityState],
        loans: &mut [BankLoan],
    ) {
        for loan in loans.iter_mut() {
            if !loan.is_active {
                continue;
            }

            let payment_due = loan.monthly_payment.min(loan.remaining_debt);
            if payment_due <= 0.01 {
                loan.is_active = false;
                continue;
            }

            let borrower_idx = loan.borrower_id.index();
            let bank_idx = loan.bank_id.index();

            if borrower_idx >= companies.len() || bank_idx >= companies.len() {
                continue;
            }

            let borrower_bal = companies[borrower_idx].balance;
            let max_payment = (borrower_bal * 0.5).max(0.0);
            let mut actual_payment = payment_due.min(max_payment);

            if actual_payment > 0.0 {
                companies[borrower_idx].balance -= actual_payment;
                companies[borrower_idx].monthly_expenses += actual_payment;
                loan.remaining_debt -= actual_payment;
            }

            let mut shortage = payment_due - actual_payment;
            if shortage > 0.01 {
                for fac in facilities.iter_mut() {
                    if fac.company_id == loan.borrower_id && fac.local_reserve > 1000.0 {
                        let from_fac = shortage.min(fac.local_reserve - 1000.0);
                        if from_fac > 0.0 {
                            fac.local_reserve -= from_fac;
                            actual_payment += from_fac;
                            loan.remaining_debt -= from_fac;
                            shortage -= from_fac;
                            if shortage <= 0.01 {
                                break;
                            }
                        }
                    }
                }
            }

            if actual_payment > 0.0 {
                companies[bank_idx].balance += actual_payment;
                let monthly_interest =
                    (loan.remaining_debt + actual_payment) * (loan.interest_rate / 12.0);
                let interest_portion = actual_payment.min(monthly_interest).max(0.0);
                companies[bank_idx].monthly_revenue += interest_portion;
            }

            loan.months_left = loan.months_left.saturating_sub(1);
            if loan.remaining_debt <= 1.0 {
                loan.is_active = false;
            } else if loan.months_left == 0 {
                let comp_id = loan.borrower_id.0 as usize;
                let has_active = comp_id < self.borrower_active_facilities.len()
                    && self.borrower_active_facilities[comp_id];
                if has_active {
                    loan.months_left = 12;
                    loan.monthly_payment = (loan.remaining_debt / 12.0).max(100.0);
                } else {
                    loan.is_active = false;
                }
            }
        }
    }

    fn collect_state_bank_dividends(
        &self,
        countries: &mut [Country],
        companies: &mut [CompanyState],
    ) {
        for comp in companies.iter_mut() {
            if comp.sector_kind == SectorKind::Financial && comp.ownership == Ownership::State {
                let profit = comp.monthly_profit();
                if profit > 1000.0 && comp.balance > 50_000.0 {
                    let div = profit * 0.5;
                    comp.balance -= div;
                    comp.monthly_expenses += div;
                    if let Some(&c_idx) = self.tag_to_country_idx.get(&comp.country_tag) {
                        countries[c_idx].economy.treasury += div;
                    }
                }
            }
        }
    }

    fn distribute_deposit_interest(
        &mut self,
        countries: &[Country],
        regions: &mut [Region],
        companies: &mut [CompanyState],
    ) {
        let num_countries = countries.len();
        self.commercial_banks.clear();
        self.commercial_banks.resize(num_countries, None);

        for (idx, comp) in companies.iter().enumerate() {
            if comp.sector_kind == SectorKind::Financial
                && matches!(comp.ownership, Ownership::Private { .. })
                && comp.balance > 10_000.0
            {
                if let Some(&c_idx) = self.tag_to_country_idx.get(&comp.country_tag) {
                    if self.commercial_banks[c_idx].is_none() {
                        self.commercial_banks[c_idx] = Some(idx);
                    }
                }
            }
        }

        let monthly_deposit_rate = 0.0025;

        for c_idx in 0..num_countries {
            let Some(bank_idx) = self.commercial_banks[c_idx] else {
                continue;
            };

            let mut total_deposits = 0.0;
            for &r_idx in &self.country_regions[c_idx] {
                let region = &mut regions[r_idx];
                for pop in &mut region.pops {
                    if pop.employed.savings > 0.0 {
                        let interest = pop.employed.savings * monthly_deposit_rate;
                        pop.employed.savings += interest;
                        total_deposits += pop.employed.savings;
                    }
                    if pop.unemployed.savings > 0.0 {
                        let interest = pop.unemployed.savings * monthly_deposit_rate;
                        pop.unemployed.savings += interest;
                        total_deposits += pop.unemployed.savings;
                    }
                }
            }

            let net_interest_margin = total_deposits * 0.0035;
            if net_interest_margin > 0.0 {
                companies[bank_idx].balance += net_interest_margin;
                companies[bank_idx].monthly_revenue += net_interest_margin;
            }
        }
    }

    fn process_consumer_banking(
        &mut self,
        countries: &[Country],
        regions: &mut [Region],
        companies: &mut [CompanyState],
    ) {
        let num_countries = countries.len();
        self.consumer_banks.clear();
        self.consumer_banks.resize(num_countries, None);

        for (idx, comp) in companies.iter().enumerate() {
            if comp.sector_kind == SectorKind::Financial
                && matches!(comp.ownership, Ownership::Private { .. })
                && comp.balance >= 25_000.0
            {
                if let Some(&c_idx) = self.tag_to_country_idx.get(&comp.country_tag) {
                    if self.consumer_banks[c_idx].is_none() {
                        self.consumer_banks[c_idx] = Some(idx);
                    }
                }
            }
        }

        for c_idx in 0..num_countries {
            let Some(bank_idx) = self.consumer_banks[c_idx] else {
                continue;
            };

            let ex_rate = self.country_exchange_rates[c_idx];
            let min_reserve = 50_000.0 * ex_rate;
            let mut total_repaid_balance = 0.0;
            let mut total_interest_earned = 0.0;

            for &r_idx in &self.country_regions[c_idx] {
                let region = &mut regions[r_idx];
                for pop in &mut region.pops {
                    for subgroup in [&mut pop.employed, &mut pop.unemployed] {
                        if subgroup.debt > 0.01 {
                            let principal_installment = (subgroup.debt / 12.0).min(subgroup.debt);
                            let monthly_interest = subgroup.debt * 0.01;
                            let total_due = principal_installment + monthly_interest;

                            let can_pay = (subgroup.savings * 0.35).min(total_due).max(0.0);
                            if can_pay > 0.0 {
                                subgroup.savings -= can_pay;
                                let interest_paid = can_pay.min(monthly_interest);
                                let principal_paid = can_pay - interest_paid;
                                subgroup.debt = (subgroup.debt - principal_paid).max(0.0);

                                total_repaid_balance += can_pay;
                                total_interest_earned += interest_paid;
                            }
                        }
                    }

                    if pop.employed.count > 0 && pop.employed.period_income > 0.0 {
                        let per_capita_savings = pop.employed.savings / (pop.employed.count as f64);
                        let per_capita_debt = pop.employed.debt / (pop.employed.count as f64);

                        let target_buffer_per_capita = 15.0 * ex_rate;
                        let max_debt_per_capita = 40.0 * ex_rate;

                        if per_capita_savings < target_buffer_per_capita
                            && per_capita_debt < max_debt_per_capita
                        {
                            let needed_total = (target_buffer_per_capita - per_capita_savings)
                                * (pop.employed.count as f64);
                            let max_borrow_total = (max_debt_per_capita - per_capita_debt)
                                * (pop.employed.count as f64);
                            let bank_liquidity_cap = (companies[bank_idx].balance * 0.04).max(0.0);

                            let loan_amount = needed_total
                                .min(max_borrow_total)
                                .min(bank_liquidity_cap)
                                .min(25_000.0 * ex_rate);

                            if loan_amount >= 10.0 * ex_rate
                                && companies[bank_idx].balance >= min_reserve + loan_amount
                            {
                                companies[bank_idx].balance -= loan_amount;
                                pop.employed.savings += loan_amount;
                                pop.employed.debt += loan_amount;
                            }
                        }
                    }
                }
            }

            if total_repaid_balance > 0.0 {
                companies[bank_idx].balance += total_repaid_balance;
                companies[bank_idx].monthly_revenue += total_interest_earned;
            }
        }
    }

    fn process_working_capital_loans(
        &mut self,
        companies: &mut [CompanyState],
        loans: &mut Vec<BankLoan>,
    ) {
        let max_comp_id = companies.iter().map(|c| c.id.0 as usize).max().unwrap_or(0);

        self.active_loan_counts.clear();
        self.active_loan_counts.resize(max_comp_id + 1, 0);

        for loan in loans.iter().filter(|l| l.is_active) {
            let id = loan.borrower_id.0 as usize;
            if id <= max_comp_id {
                self.active_loan_counts[id] += 1;
            }
        }

        for comp_idx in 0..companies.len() {
            let comp = &companies[comp_idx];
            if comp.sector_kind == SectorKind::Financial {
                continue;
            }
            let comp_id = comp.id.0 as usize;
            if comp_id >= self.company_has_facilities.len() || !self.company_has_facilities[comp_id]
            {
                continue;
            }
            let active_loans = if comp_id < self.active_loan_counts.len() {
                self.active_loan_counts[comp_id]
            } else {
                0
            };
            if active_loans >= 2 {
                continue;
            }

            let ex_rate = self
                .tag_to_country_idx
                .get(&comp.country_tag)
                .map(|&c_idx| self.country_exchange_rates[c_idx])
                .unwrap_or(1.0);
            let safety_threshold = 25_000.0 * ex_rate;

            if comp.balance < safety_threshold {
                let needed_loan =
                    (comp.monthly_expenses * 0.5).clamp(25_000.0 * ex_rate, 500_000.0 * ex_rate);
                let country_tag = comp.country_tag;
                let _ = BankLoan::request_corporate_loan(
                    comp_idx,
                    needed_loan,
                    country_tag,
                    companies,
                    loans,
                );
            }
        }
    }

    fn process_state_recapitalization(
        &self,
        companies: &mut [CompanyState],
        countries: &mut [Country],
    ) {
        for country in countries.iter_mut() {
            let policy = country.economy.bailout_policy;
            if policy == BailoutPolicy::Disabled {
                continue;
            }

            let ex_rate = country.economy.exchange_rate.max(0.001);
            let min_treasury_reserve = 50_000.0 * ex_rate;

            if country.economy.treasury <= min_treasury_reserve {
                continue;
            }

            let available_surplus = country.economy.treasury - min_treasury_reserve;
            let max_subsidy_budget =
                available_surplus * country.economy.max_subsidy_budget_ratio.clamp(0.0, 1.0);
            let mut spent_this_month = 0.0;

            for comp in companies.iter_mut() {
                if spent_this_month >= max_subsidy_budget {
                    break;
                }
                if comp.country_tag != country.tag {
                    continue;
                }

                if comp.sector_kind == SectorKind::Financial && comp.ownership == Ownership::State {
                    let bank_safety_threshold = 100_000.0 * ex_rate;
                    if comp.balance < bank_safety_threshold {
                        let needed = bank_safety_threshold - comp.balance;
                        let remaining_budget = max_subsidy_budget - spent_this_month;
                        let grant = needed.min(remaining_budget).min(50_000.0 * ex_rate);
                        if grant > 0.0 && country.economy.treasury >= min_treasury_reserve + grant {
                            country.economy.treasury -= grant;
                            comp.balance += grant;
                            comp.monthly_revenue += grant;
                            spent_this_month += grant;
                        }
                    }
                    continue;
                }

                let comp_id = comp.id.0 as usize;
                if comp_id >= self.company_has_facilities.len()
                    || !self.company_has_facilities[comp_id]
                {
                    continue;
                }

                match comp.ownership {
                    Ownership::State => {
                        let state_safety_threshold = 10_000.0 * ex_rate;
                        if comp.balance < state_safety_threshold {
                            let needed = state_safety_threshold - comp.balance;
                            let remaining_budget = max_subsidy_budget - spent_this_month;
                            let grant = needed.min(remaining_budget).min(25_000.0 * ex_rate);
                            if grant > 0.0
                                && country.economy.treasury >= min_treasury_reserve + grant
                            {
                                country.economy.treasury -= grant;
                                comp.balance += grant;
                                comp.monthly_revenue += grant;
                                spent_this_month += grant;
                            }
                        }
                    }
                    Ownership::Private { .. } => {
                        let is_allowed = matches!(
                            policy,
                            BailoutPolicy::StrategicAndState | BailoutPolicy::Universal
                        );
                        if !is_allowed {
                            continue;
                        }

                        let private_critical_threshold = 2_000.0 * ex_rate;

                        if comp.balance < private_critical_threshold {
                            let needed = (10_000.0 * ex_rate) - comp.balance;
                            let remaining_budget = max_subsidy_budget - spent_this_month;
                            let bailout = needed.min(remaining_budget).min(10_000.0 * ex_rate);
                            if bailout > 0.0
                                && country.economy.treasury >= min_treasury_reserve + bailout
                            {
                                country.economy.treasury -= bailout;
                                comp.balance += bailout;
                                comp.monthly_revenue += bailout;
                                spent_this_month += bailout;
                            }
                        }
                    }
                    Ownership::Regional(_) => {}
                }
            }
        }
    }
}
