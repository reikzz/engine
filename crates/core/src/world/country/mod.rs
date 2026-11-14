mod tag;

use crate::economy::market::NationalMarket;
pub use crate::world::construction::{ConstructionProject, ConstructionTarget};
use crate::world::map::RegionId;
use crate::world::structure::StateStructure;
use crate::world::tech::TechnologyId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub use tag::CountryTag;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WelfarePolicy {
    pub max_tax_share: f64,
    pub min_daily_benefit: f64,
    pub max_daily_benefit: f64,
}

impl Default for WelfarePolicy {
    fn default() -> Self {
        Self {
            max_tax_share: 0.25,
            min_daily_benefit: 0.20,
            max_daily_benefit: 1.50,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BailoutPolicy {
    Disabled,
    #[default]
    StateOnly,
    StrategicAndState,
    Universal,
}

#[derive(Debug)]
pub struct Country {
    pub tag: CountryTag,
    pub regions: Vec<RegionId>,
    pub economy: CountryEconomy,
    pub market: NationalMarket,
    pub structures: Vec<StateStructure>,
    pub construction_queue: Vec<ConstructionProject>,
    pub unlocked_technologies: BTreeSet<TechnologyId>,
    pub active_research: Option<TechnologyId>,
    pub research_progress: f64,
    pub last_period_science_output: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryEconomy {
    pub currency_code: String,
    pub currency_symbol: String,
    pub exchange_rate: f64,

    pub treasury: f64,
    pub national_debt: f64,
    pub nominal_gdp: f64,

    pub last_period_tax_revenue: f64,
    pub last_period_expenses: f64,
    pub last_period_tariff_revenue: f64,

    pub corporate_tax: f64,
    pub income_tax: f64,
    pub tariff_rate: f64,

    pub bailout_policy: BailoutPolicy,
    pub max_subsidy_budget_ratio: f64,
    pub welfare: WelfarePolicy,
}

impl Default for CountryEconomy {
    fn default() -> Self {
        Self {
            currency_code: "USD".to_string(),
            currency_symbol: "$".to_string(),
            exchange_rate: 1.0,
            treasury: 0.0,
            national_debt: 0.0,
            nominal_gdp: 0.0,
            last_period_tax_revenue: 0.0,
            last_period_expenses: 0.0,
            last_period_tariff_revenue: 0.0,
            corporate_tax: 0.20,
            income_tax: 0.15,
            tariff_rate: 0.05,
            bailout_policy: BailoutPolicy::StrategicAndState,
            max_subsidy_budget_ratio: 0.05,
            welfare: WelfarePolicy::default(),
        }
    }
}

impl Country {
    pub fn new(tag: CountryTag, market: NationalMarket) -> Self {
        Self {
            tag,
            economy: CountryEconomy::default(),
            regions: Vec::new(),
            market,
            structures: Vec::new(),
            construction_queue: Vec::new(),
            unlocked_technologies: BTreeSet::new(),
            active_research: None,
            research_progress: 0.0,
            last_period_science_output: 0.0,
        }
    }

    pub fn with_economy(tag: CountryTag, economy: CountryEconomy, market: NationalMarket) -> Self {
        Self {
            tag,
            economy,
            regions: Vec::new(),
            market,
            structures: Vec::new(),
            construction_queue: Vec::new(),
            unlocked_technologies: BTreeSet::new(),
            active_research: None,
            research_progress: 0.0,
            last_period_science_output: 0.0,
        }
    }

    pub fn has_technology(&self, id: TechnologyId) -> bool {
        self.unlocked_technologies.contains(&id)
    }

    pub fn unlock_technology(&mut self, id: TechnologyId) {
        self.unlocked_technologies.insert(id);
    }

    pub fn tick(&mut self) {}
}
