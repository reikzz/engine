use crate::economy::{FacilityId, Stockpile};
use crate::world::country::CountryTag;
use crate::world::map::ProvinceId;
use crate::world::{Pop, RegionStructure};
use serde::{Deserialize, Serialize};
use strum::EnumCount;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RegionId(pub u16);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumCount, strum::FromRepr,
)]
#[repr(usize)]
pub enum RegionModifier {
    PopulationGrowth,
    MortalityRate,
    LaborProductivity,
    EducationEfficiency,
    InfrastructureCapacity,
    CrimeRate,
    DisasterLossReduction,
    SocialStability,
    TaxEfficiency,
    MarketAccess,
}

#[derive(Debug)]
pub struct Region {
    pub id: RegionId,

    pub owner: Option<CountryTag>,
    pub stockpile: Stockpile,

    pub local_treasury: f64,

    pub provinces: Vec<ProvinceId>,
    pub neighbors: Vec<RegionId>,

    pub facilities: Vec<FacilityId>,

    pub structures: Vec<RegionStructure>,

    pub pops: Vec<Pop>,

    pub infrastructure_level: f64,
    pub infrastructure_capacity: f64,
    pub infrastructure_usage: f64,
    pub market_access: f64,
    pub local_prices: Vec<f64>,
    pub local_supplied: Vec<f64>,
    pub local_demanded: Vec<f64>,
    pub local_buy_fulfillment: Vec<f64>,
    pub local_sell_fulfillment: Vec<f64>,

    pub modifiers: RegionModifiers,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionModifiers {
    values: [f64; RegionModifier::COUNT],
}

impl Default for RegionModifiers {
    fn default() -> Self {
        Self {
            values: [0.0; RegionModifier::COUNT],
        }
    }
}

impl RegionModifiers {
    #[inline]
    pub fn clear(&mut self) {
        self.values.fill(0.0);
    }

    #[inline]
    pub fn add(&mut self, modifier: RegionModifier, value: f64) {
        self.values[modifier as usize] += value;
    }

    #[inline]
    pub fn get(&self, modifier: RegionModifier) -> f64 {
        self.values[modifier as usize]
    }
}

impl Region {
    pub fn new(
        id: RegionId,
        owner: Option<CountryTag>,
        stockpile: Stockpile,
        infrastructure_level: f64,
    ) -> Self {
        Self {
            id,
            owner,
            stockpile,
            local_treasury: 0.0,
            provinces: Vec::new(),
            neighbors: Vec::new(),
            facilities: Vec::new(),
            structures: Vec::new(),
            infrastructure_level,
            infrastructure_capacity: 100.0,
            infrastructure_usage: 0.0,
            market_access: 1.0,
            local_prices: Vec::new(),
            local_supplied: Vec::new(),
            local_demanded: Vec::new(),
            local_buy_fulfillment: Vec::new(),
            local_sell_fulfillment: Vec::new(),
            pops: Vec::new(),
            modifiers: RegionModifiers::default(),
        }
    }

    #[inline]
    pub fn is_neighbor(&self, other_id: RegionId) -> bool {
        self.neighbors.contains(&other_id)
    }

    pub fn add_neighbor(&mut self, neighbor_id: RegionId) {
        if neighbor_id != self.id && !self.neighbors.contains(&neighbor_id) {
            self.neighbors.push(neighbor_id);
        }
    }

    #[inline]
    pub fn reset_local_market_tracking(&mut self, resource_count: usize) {
        if self.local_supplied.len() != resource_count {
            self.local_supplied.resize(resource_count, 0.0);
        } else {
            self.local_supplied.fill(0.0);
        }

        if self.local_demanded.len() != resource_count {
            self.local_demanded.resize(resource_count, 0.0);
        } else {
            self.local_demanded.fill(0.0);
        }

        if self.local_prices.len() != resource_count {
            self.local_prices.resize(resource_count, 0.0);
        }

        if self.local_buy_fulfillment.len() != resource_count {
            self.local_buy_fulfillment.resize(resource_count, 1.0);
        }

        if self.local_sell_fulfillment.len() != resource_count {
            self.local_sell_fulfillment.resize(resource_count, 1.0);
        }
    }

    pub fn update_local_market(&mut self, base_prices: &[f64], national_prices: &[f64]) {
        let count = base_prices.len();
        if self.local_prices.len() != count {
            self.local_prices.resize(count, 0.0);
        }
        if self.local_buy_fulfillment.len() != count {
            self.local_buy_fulfillment.resize(count, 1.0);
        }
        if self.local_sell_fulfillment.len() != count {
            self.local_sell_fulfillment.resize(count, 1.0);
        }

        for idx in 0..count {
            let s = self.local_supplied.get(idx).copied().unwrap_or(0.0);
            let d = self.local_demanded.get(idx).copied().unwrap_or(0.0);
            let base_p = base_prices[idx];
            let nat_p = national_prices.get(idx).copied().unwrap_or(base_p);

            let eps = 1e-6;
            if s > eps && d > eps {
                self.local_sell_fulfillment[idx] = (d / s).min(1.0);
                self.local_buy_fulfillment[idx] = (s / d).min(1.0);
            } else if s > eps && d <= eps {
                self.local_sell_fulfillment[idx] = 0.0;
                self.local_buy_fulfillment[idx] = 1.0;
            } else if s <= eps && d > eps {
                self.local_sell_fulfillment[idx] = 1.0;
                self.local_buy_fulfillment[idx] = 0.0;
            } else {
                self.local_sell_fulfillment[idx] = 1.0;
                self.local_buy_fulfillment[idx] = 1.0;
            }

            if s <= eps && d <= eps {
                self.local_prices[idx] = nat_p;
            } else {
                let ratio = (d / s.max(eps)).clamp(0.2, 5.0);
                let pure_local = (base_p * ratio.powf(0.5)).clamp(base_p * 0.3, base_p * 3.0);
                self.local_prices[idx] = pure_local;
            }
        }
    }

    #[inline]
    pub fn effective_price(&self, resource_idx: usize, national_price: f64) -> f64 {
        if let Some(&local_price) = self.local_prices.get(resource_idx) {
            if local_price > 0.0 {
                return (self.market_access * national_price
                    + (1.0 - self.market_access) * local_price)
                    .max(0.01);
            }
        }
        national_price
    }

    #[inline]
    pub fn effective_buy_fulfillment(&self, resource_idx: usize, national_fulfillment: f64) -> f64 {
        let local = self
            .local_buy_fulfillment
            .get(resource_idx)
            .copied()
            .unwrap_or(1.0);
        (self.market_access * national_fulfillment + (1.0 - self.market_access) * local)
            .clamp(0.0, 1.0)
    }

    #[inline]
    pub fn effective_sell_fulfillment(
        &self,
        resource_idx: usize,
        national_fulfillment: f64,
    ) -> f64 {
        let local = self
            .local_sell_fulfillment
            .get(resource_idx)
            .copied()
            .unwrap_or(1.0);
        (self.market_access * national_fulfillment + (1.0 - self.market_access) * local)
            .clamp(0.0, 1.0)
    }

    pub fn population(&self) -> u64 {
        self.pops.iter().map(|p| p.count()).sum()
    }
}
