use crate::economy::{CompanyId, CompanyState, Ownership, SectorKind};
use crate::world::CountryTag;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankLoan {
    pub id: u64,
    pub bank_id: CompanyId,
    pub borrower_id: CompanyId,
    pub country_tag: CountryTag,
    pub principal: f64,
    pub remaining_debt: f64,
    pub monthly_payment: f64,
    pub interest_rate: f64,
    pub months_left: u32,
    pub is_active: bool,
}

impl BankLoan {
    pub fn new(
        id: u64,
        bank_id: CompanyId,
        borrower_id: CompanyId,
        country_tag: CountryTag,
        principal: f64,
        annual_rate: f64,
        months: u32,
    ) -> Self {
        let months_u = months.max(1);
        let monthly_rate = annual_rate / 12.0;
        let monthly_payment = if monthly_rate > 1e-6 {
            let factor = (1.0 + monthly_rate).powi(months_u as i32);
            principal * monthly_rate * factor / (factor - 1.0)
        } else {
            principal / (months_u as f64)
        };

        let total_debt = monthly_payment * (months_u as f64);

        Self {
            id,
            bank_id,
            borrower_id,
            country_tag,
            principal,
            remaining_debt: total_debt,
            monthly_payment,
            interest_rate: annual_rate,
            months_left: months_u,
            is_active: true,
        }
    }

    pub fn request_corporate_loan(
        borrower_idx: usize,
        amount: f64,
        country_tag: CountryTag,
        companies: &mut [CompanyState],
        loans: &mut Vec<BankLoan>,
    ) -> bool {
        if borrower_idx >= companies.len() || amount <= 0.0 {
            return false;
        }

        let borrower_id = companies[borrower_idx].id;

        let priv_bank_idx = companies.iter().position(|c| {
            c.country_tag == country_tag
                && c.sector_kind == SectorKind::Financial
                && matches!(c.ownership, Ownership::Private { .. })
                && c.balance >= 1000.0
        });

        if let Some(b_idx) = priv_bank_idx {
            let lend_amount = amount.min(companies[b_idx].balance * 0.7);
            if lend_amount >= 1000.0 {
                let bank_id = companies[b_idx].id;
                companies[b_idx].balance -= lend_amount;
                companies[borrower_idx].balance += lend_amount;
                let loan_id = loans.len() as u64 + 1;
                loans.push(Self::new(
                    loan_id,
                    bank_id,
                    borrower_id,
                    country_tag,
                    lend_amount,
                    0.08,
                    36,
                ));
                return true;
            }
        }

        let state_bank_idx = companies.iter().position(|c| {
            c.country_tag == country_tag
                && c.sector_kind == SectorKind::Financial
                && c.ownership == Ownership::State
                && c.balance >= 1000.0
        });

        if let Some(b_idx) = state_bank_idx {
            let lend_amount = amount.min(companies[b_idx].balance * 0.7);
            if lend_amount >= 1000.0 {
                let bank_id = companies[b_idx].id;
                companies[b_idx].balance -= lend_amount;
                companies[borrower_idx].balance += lend_amount;
                let loan_id = loans.len() as u64 + 1;
                loans.push(Self::new(
                    loan_id,
                    bank_id,
                    borrower_id,
                    country_tag,
                    lend_amount,
                    0.05,
                    36,
                ));
                return true;
            }
        }

        false
    }
}
