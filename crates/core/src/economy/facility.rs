use crate::CountryTag;
use crate::economy::{CompanyId, ProductionLineState, ResourceId, SectorKind, Stockpile};
use crate::world::map::RegionId;
use crate::world::time::GameDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FacilityId(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDelivery {
    pub resource_id: ResourceId,
    pub amount: f64,
    pub arrival_date: GameDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacilityState {
    pub id: FacilityId,
    pub company_id: CompanyId,
    pub country_tag: CountryTag,
    pub sector_kind: SectorKind,
    pub region_id: RegionId,
    pub stockpile: Stockpile,
    pub lines: Vec<ProductionLineState>,
    pub is_priority_development_zone: bool,
    pub local_reserve: f64,
    pub pending_deliveries: Vec<PendingDelivery>,

    pub period_revenue: f64,
    pub period_material_cost: f64,
    pub period_wage_cost: f64,
}

impl FacilityState {
    #[inline]
    pub fn pending_amount(&self, resource_id: ResourceId) -> f64 {
        self.pending_deliveries
            .iter()
            .filter(|d| d.resource_id == resource_id)
            .map(|d| d.amount)
            .sum()
    }

    #[inline]
    pub fn net_profit(&self) -> f64 {
        self.period_revenue - self.period_material_cost - self.period_wage_cost
    }

    #[inline]
    pub fn reset_period_finances(&mut self) {
        self.period_revenue = 0.0;
        self.period_material_cost = 0.0;
        self.period_wage_cost = 0.0;
    }
}
