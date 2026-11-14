use crate::CountryTag;
use crate::economy::SectorKind;
use crate::world::map::RegionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CompanyId(pub u32);

impl CompanyId {
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Ownership {
    State,
    Private { shareholders_group_id: u32 },
    Regional(RegionId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyState {
    pub id: CompanyId,
    pub name: String,
    pub country_tag: CountryTag,
    pub ownership: Ownership,
    pub sector_kind: SectorKind,
    pub balance: f64,
    pub reinvestment_ratio: f64,

    pub monthly_revenue: f64,
    pub monthly_expenses: f64,
    pub last_month_profit: f64,
}

impl CompanyState {
    #[inline]
    pub fn monthly_profit(&self) -> f64 {
        self.monthly_revenue - self.monthly_expenses
    }
}
