use crate::economy::ResourceId;
use crate::economy::resource::Resource;
use crate::registry::Registry;

#[derive(Debug, Clone)]
pub struct GlobalCommodityQuote {
    pub benchmark_price: f64,
    pub daily_volume: f64,
    pub global_surplus: f64,
    pub global_deficit: f64,
}

impl GlobalCommodityQuote {
    pub fn new(base_price: f64) -> Self {
        Self {
            benchmark_price: base_price,
            daily_volume: 0.0,
            global_surplus: 0.0,
            global_deficit: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GlobalMarket {
    quotes: Vec<GlobalCommodityQuote>,
}

impl GlobalMarket {
    pub fn new(base_prices: &[f64]) -> Self {
        Self {
            quotes: base_prices
                .iter()
                .map(|&p| GlobalCommodityQuote::new(p))
                .collect(),
        }
    }

    pub fn from_registry(resources: &Registry<ResourceId, Resource>) -> Self {
        let prices: Vec<f64> = resources.all().iter().map(|r| r.base_price).collect();
        Self::new(&prices)
    }

    #[inline]
    fn idx(id: ResourceId) -> usize {
        id.0 as usize
    }

    #[inline]
    pub fn get(&self, id: ResourceId) -> Option<&GlobalCommodityQuote> {
        self.quotes.get(Self::idx(id))
    }

    #[inline]
    pub fn get_mut(&mut self, id: ResourceId) -> Option<&mut GlobalCommodityQuote> {
        self.quotes.get_mut(Self::idx(id))
    }

    #[inline]
    pub fn benchmark_price(&self, id: ResourceId) -> f64 {
        self.get(id).map_or(0.0, |q| q.benchmark_price)
    }

    pub fn reset_daily_stats(&mut self) {
        for q in &mut self.quotes {
            q.daily_volume = 0.0;
            q.global_surplus = 0.0;
            q.global_deficit = 0.0;
        }
    }

    pub fn record_trade(&mut self, id: ResourceId, volume: f64, price: f64) {
        if let Some(q) = self.get_mut(id) {
            q.daily_volume += volume;
            q.benchmark_price = q.benchmark_price * 0.95 + price * 0.05;
        }
    }

    pub fn record_market_conditions(&mut self, id: ResourceId, surplus: f64, deficit: f64) {
        if let Some(q) = self.get_mut(id) {
            q.global_surplus += surplus;
            q.global_deficit += deficit;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (ResourceId, &GlobalCommodityQuote)> {
        self.quotes
            .iter()
            .enumerate()
            .map(|(idx, q)| (ResourceId(idx as u16), q))
    }
}
