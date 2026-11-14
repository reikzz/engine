use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct CountrySnapshot {
    pub tag: String,
    pub currency_symbol: String,
    pub treasury: f64,
    pub national_debt: f64,
    pub total_population: u64,
    pub employed_population: u64,
    pub nominal_gdp: f64,
    pub corporate_tax: f64,
    pub income_tax: f64,
    pub tariff_rate: f64,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct RegionSnapshot {
    pub region_id: u16,
    pub owner: Option<String>,
    pub total_population: u64,
    pub employed_population: u64,
    pub infrastructure_level: f64,
    pub infrastructure_capacity: f64,
    pub market_access: f64,
    pub average_price: f64,
    pub facility_count: usize,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct FacilityDetailSnapshot {
    pub facility_id: u32,
    pub name: String,
    pub sector: String,
    pub workforce_count: u64,
    pub workforce_capacity: u64,
    pub daily_output: f64,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct InspectedRegionDetail {
    pub region_id: u16,
    pub name: String,
    pub country_tag: String,
    pub terrain: String,
    pub total_pop: u64,
    pub employed_pop: u64,
    pub satisfaction: f32,
    pub radicalism: f32,
    pub infrastructure_level: f64,
    pub infrastructure_capacity: f64,
    pub infrastructure_load: f64,
    pub market_access: f64,
    pub average_price: f64,
    pub facilities: Vec<FacilityDetailSnapshot>,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ConstructionOptionSnapshot {
    pub recipe_id: u16,
    pub key: String,
    pub name: String,
    pub sector: String,
    pub build_time_days: u32,
    pub cost_text: String,
    pub materials_summary: String,
    pub tech_unlocked: bool,
    pub required_tech_name: String,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct TechItemSnapshot {
    pub id: u16,
    pub key: String,
    pub name: String,
    pub category: String,
    pub cost_points: u32,
    pub progress_ratio: f32,
    pub status: String,
    pub is_active: bool,
    pub is_completed: bool,
    pub can_research: bool,
    pub unlocks_summary: String,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ProvinceSnapshot {
    pub province_id: u16,
    pub region_id: u16,
    pub terrain: String,
    pub controller: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SystemMetricSnapshot {
    pub name: String,
    pub last_duration_micros: u64,
    pub average_duration_micros: f64,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ResourcePriceSnapshot {
    pub id: u16,
    pub name: String,
    pub base_price: f64,
    pub local_price: f64,
    pub supply: f64,
    pub demand: f64,
    pub global_price: f64,
    pub global_volume: f64,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct CompanySnapshot {
    pub id: u32,
    pub name: String,
    pub country_tag: String,
    pub sector: String,
    pub balance: f64,
    pub monthly_revenue: f64,
    pub monthly_expenses: f64,
    pub last_month_profit: f64,
}

#[derive(Debug, Clone, Default)]
pub struct WorldSnapshot {
    pub date_display: String,
    pub total_ticks: u64,
    pub ticks_per_second: f64,
    pub countries: BTreeMap<String, CountrySnapshot>,
    pub regions: BTreeMap<u16, RegionSnapshot>,
    pub provinces: BTreeMap<u16, ProvinceSnapshot>,
    pub system_metrics: Vec<SystemMetricSnapshot>,
    pub local_market_prices: BTreeMap<String, Vec<ResourcePriceSnapshot>>,
    pub global_market_prices: Vec<ResourcePriceSnapshot>,
    pub companies: Vec<CompanySnapshot>,
    pub inspected_region_detail: Option<InspectedRegionDetail>,
    pub construction_options: Vec<ConstructionOptionSnapshot>,
    pub tech_tree: Vec<TechItemSnapshot>,
    pub active_tech_name: Option<String>,
    pub active_tech_category: Option<String>,
    pub active_tech_progress: f32,
}
