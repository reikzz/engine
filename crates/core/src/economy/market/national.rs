use crate::economy::ResourceId;
use crate::economy::market::{MarketConfig, ResourceMarket};
use crate::economy::resource::Resource;
use crate::registry::Registry;

#[derive(Debug, Clone)]
pub struct NationalMarket {
    resources: Vec<ResourceMarket>,
    pub config: MarketConfig,
}

impl NationalMarket {
    pub fn new(base_prices: &[f64]) -> Self {
        Self {
            resources: base_prices
                .iter()
                .map(|&p| ResourceMarket::new(p))
                .collect(),
            config: MarketConfig::default(),
        }
    }

    pub fn from_registry(resources: &Registry<ResourceId, Resource>) -> Self {
        Self::from_registry_scaled(resources, 1.0)
    }

    pub fn from_registry_scaled(
        resources: &Registry<ResourceId, Resource>,
        exchange_rate: f64,
    ) -> Self {
        let prices: Vec<f64> = resources
            .all()
            .iter()
            .map(|r| r.base_price * exchange_rate)
            .collect();
        Self::new(&prices)
    }

    #[inline]
    fn idx(id: ResourceId) -> usize {
        id.0 as usize
    }

    #[inline]
    pub fn get(&self, id: ResourceId) -> Option<&ResourceMarket> {
        self.resources.get(Self::idx(id))
    }

    #[inline]
    pub fn get_mut(&mut self, id: ResourceId) -> Option<&mut ResourceMarket> {
        self.resources.get_mut(Self::idx(id))
    }

    #[inline]
    pub fn price(&self, id: ResourceId) -> f64 {
        let r = self.get(id);

        r.map_or(0.0, |r| r.price)
    }

    #[inline]
    pub fn record_supply(&mut self, id: ResourceId, amount: f64) {
        if let Some(r) = self.get_mut(id) {
            r.supplied += amount.max(0.0);
        }
    }

    #[inline]
    pub fn record_demand(&mut self, id: ResourceId, amount: f64) {
        if let Some(r) = self.get_mut(id) {
            r.demanded += amount.max(0.0);
        }
    }

    pub fn set_corridor(&mut self, id: ResourceId, min: f64, max: f64) {
        if let Some(r) = self.get_mut(id) {
            r.min_price = min.min(max);
            r.max_price = max.max(min);
        }
    }

    pub fn update_prices(&mut self) {
        let cfg = self.config;
        for r in &mut self.resources {
            r.update(&cfg);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (ResourceId, &ResourceMarket)> {
        self.resources
            .iter()
            .enumerate()
            .map(|(idx, r)| (ResourceId(idx as u16), r))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (ResourceId, &mut ResourceMarket)> {
        self.resources
            .iter_mut()
            .enumerate()
            .map(|(idx, r)| (ResourceId(idx as u16), r))
    }
}
