use crate::CountryTag;
use crate::economy::CompanyId;
use crate::world::map::RegionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BudgetSource {
    Federal(CountryTag),
    Regional(RegionId),
    Shared {
        country: CountryTag,
        region: RegionId,
        federal_share: f32,
    },
    Company(CompanyId),
}
