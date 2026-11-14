mod global;
mod national;

pub use global::{GlobalCommodityQuote, GlobalMarket};
pub use national::NationalMarket;

const EPS: f64 = 1e-9;

#[derive(Debug, Clone, Copy)]
pub struct MarketConfig {
    pub elasticity: f64,
    pub smoothing: f64,

    pub ratio_min: f64,
    pub ratio_max: f64,
}

impl Default for MarketConfig {
    fn default() -> Self {
        Self {
            elasticity: 0.5,
            smoothing: 0.1,
            ratio_min: 0.2,
            ratio_max: 5.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResourceMarket {
    pub base_price: f64,
    pub price: f64,

    pub supplied: f64,
    pub demanded: f64,

    pub last_supplied: f64,
    pub last_demanded: f64,

    pub sell_fulfillment: f64,
    pub buy_fulfillment: f64,

    pub min_price: f64,
    pub max_price: f64,

    pub revenue_pool: f64,
}

impl ResourceMarket {
    pub fn new(base_price: f64) -> Self {
        Self {
            base_price,
            price: base_price,
            supplied: 0.0,
            demanded: 0.0,
            last_supplied: 0.0,
            last_demanded: 0.0,
            sell_fulfillment: 1.0,
            buy_fulfillment: 1.0,
            revenue_pool: 0.0,
            min_price: base_price * 0.3,
            max_price: base_price * 3.0,
        }
    }

    pub fn update(&mut self, cfg: &MarketConfig) {
        if self.supplied > EPS && self.demanded > EPS {
            self.sell_fulfillment = (self.demanded / self.supplied).min(1.0);
            self.buy_fulfillment = (self.supplied / self.demanded).min(1.0);
        } else if self.supplied > EPS && self.demanded <= EPS {
            self.sell_fulfillment = 0.0;
            self.buy_fulfillment = 1.0;
        } else if self.supplied <= EPS && self.demanded > EPS {
            self.sell_fulfillment = 1.0;
            self.buy_fulfillment = 0.0;
        } else {
            self.sell_fulfillment = 1.0;
            self.buy_fulfillment = 1.0;
        }

        let ratio = if self.supplied <= EPS && self.demanded <= EPS {
            1.0
        } else {
            (self.demanded / self.supplied.max(EPS)).clamp(cfg.ratio_min, cfg.ratio_max)
        };

        let target =
            (self.base_price * ratio.powf(cfg.elasticity)).clamp(self.min_price, self.max_price);

        self.price += (target - self.price) * cfg.smoothing;
        self.price = self.price.clamp(self.min_price, self.max_price);

        self.last_supplied = self.supplied;
        self.last_demanded = self.demanded;

        self.supplied = 0.0;
        self.demanded = 0.0;
    }
}
