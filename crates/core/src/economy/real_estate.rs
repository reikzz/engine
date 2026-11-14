use crate::CountryTag;
use crate::economy::CompanyId;
use crate::world::map::RegionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RealEstateId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RealEstateKind {
    CommercialOffice,
    ResidentialHousing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealEstateProperty {
    pub id: RealEstateId,
    pub kind: RealEstateKind,
    pub owner_company_id: Option<CompanyId>,
    pub region_id: RegionId,
    pub country_tag: CountryTag,
    pub total_units: u32,
    pub occupied_units: u32,
    pub base_rent_per_unit: f64,
    pub current_rent_per_unit: f64,
    pub condition: f64,
    pub period_revenue: f64,
    pub period_maintenance_cost: f64,
}

impl RealEstateProperty {
    pub fn new(
        id: RealEstateId,
        kind: RealEstateKind,
        owner_company_id: Option<CompanyId>,
        region_id: RegionId,
        country_tag: CountryTag,
        total_units: u32,
        base_rent_per_unit: f64,
    ) -> Self {
        Self {
            id,
            kind,
            owner_company_id,
            region_id,
            country_tag,
            total_units,
            occupied_units: 0,
            base_rent_per_unit,
            current_rent_per_unit: base_rent_per_unit,
            condition: 1.0,
            period_revenue: 0.0,
            period_maintenance_cost: 0.0,
        }
    }

    #[inline]
    pub fn occupancy_ratio(&self) -> f64 {
        if self.total_units == 0 {
            0.0
        } else {
            (self.occupied_units as f64 / self.total_units as f64).clamp(0.0, 1.0)
        }
    }

    pub fn update_rent_price(&mut self) {
        let ratio = self.occupancy_ratio();
        let multiplier = 0.5 + 1.5 * ratio * ratio;
        self.current_rent_per_unit = (self.base_rent_per_unit * multiplier).max(1.0);
    }

    pub fn reset_period_finances(&mut self) {
        self.period_revenue = 0.0;
        self.period_maintenance_cost = 0.0;
    }
}
