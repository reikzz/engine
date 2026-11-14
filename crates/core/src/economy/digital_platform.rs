use crate::CountryTag;
use crate::economy::{CompanyId, FacilityId};
use crate::world::map::RegionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DigitalPlatformId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DigitalPlatformKind {
    ConsumerServices,
    EnterpriseCloud,
    BusinessSoftware,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigitalPlatform {
    pub id: DigitalPlatformId,
    pub company_id: CompanyId,
    pub facility_id: FacilityId,
    pub region_id: RegionId,
    pub country_tag: CountryTag,
    pub name: String,
    pub kind: DigitalPlatformKind,
    pub active_subscribers: u64,
    pub max_subscribers: u64,
    pub monthly_fee: f64,
    pub quality: f64,
    pub monthly_revenue: f64,
    pub is_active: bool,
}

impl DigitalPlatform {
    pub fn new(
        id: DigitalPlatformId,
        company_id: CompanyId,
        facility_id: FacilityId,
        region_id: RegionId,
        country_tag: CountryTag,
        name: String,
        kind: DigitalPlatformKind,
        max_subscribers: u64,
        monthly_fee: f64,
    ) -> Self {
        Self {
            id,
            company_id,
            facility_id,
            region_id,
            country_tag,
            name,
            kind,
            active_subscribers: 0,
            max_subscribers,
            monthly_fee,
            quality: 1.0,
            monthly_revenue: 0.0,
            is_active: true,
        }
    }

    pub fn available_capacity(&self) -> u64 {
        if !self.is_active {
            return 0;
        }
        self.max_subscribers.saturating_sub(self.active_subscribers)
    }

    pub fn attractiveness(&self, foreign_exchange_rate: f64) -> f64 {
        if !self.is_active || self.monthly_fee <= 0.0 {
            return 0.0;
        }
        let normalized_price = (self.monthly_fee * foreign_exchange_rate).max(0.01);
        (self.quality * 10.0 / normalized_price).clamp(0.01, 100.0)
    }
}
