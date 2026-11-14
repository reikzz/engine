use crate::economy::finance::BudgetSource;
use crate::economy::{Ownership, Priority, RealEstateKind, SectorKind};
use crate::loader::fs_utils::deserialize_optional;
use crate::world::StructureKind;
use crate::world::map::TerrainType;
use crate::world::{BailoutPolicy, WelfarePolicy};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub(super) struct ScenarioMeta {
    pub start_date: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct CountryDto {
    pub tag: String,

    #[serde(default)]
    pub economy: CountryEconomyDto,

    #[serde(default)]
    pub structures: Vec<StateStructureDto>,

    #[serde(default)]
    pub technologies: Vec<String>,

    #[serde(default)]
    pub active_research: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProductionLineDto {
    pub recipe_key: String,
    pub installed_capacity: u32,
    #[serde(default, deserialize_with = "deserialize_optional")]
    pub natural_max_capacity: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_optional")]
    pub target_capacity_cap: Option<u32>,
    pub priority: Priority,
    pub target_wage: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct FacilityDto {
    pub company_id: u32,
    pub sector_kind: SectorKind,
    #[serde(default, deserialize_with = "deserialize_optional")]
    pub country_tag: Option<String>,
    pub lines: Vec<ProductionLineDto>,
    #[serde(default)]
    pub is_priority_development_zone: bool,
    #[serde(default)]
    pub local_reserve: f64,
    #[serde(default)]
    pub period_revenue: f64,
    #[serde(default)]
    pub period_material_cost: f64,
    #[serde(default)]
    pub period_wage_cost: f64,
    #[serde(default)]
    pub stockpile: Option<Vec<(String, f64)>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RegionDto {
    pub id: u16,

    #[serde(default, deserialize_with = "deserialize_optional")]
    pub owner: Option<String>,

    pub infrastructure_level: f64,

    #[serde(default)]
    pub neighbors: Vec<u16>,

    #[serde(default)]
    pub facilities: Vec<FacilityDto>,

    #[serde(default)]
    pub structures: Vec<RegionStructureDto>,

    #[serde(default)]
    pub real_estate: Vec<RealEstateDto>,
}

#[derive(Debug, Deserialize)]
pub(super) struct RealEstateDto {
    pub kind: RealEstateKind,
    #[serde(default)]
    pub owner_company_id: Option<u32>,
    pub total_units: u32,
    pub base_rent: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ProvinceDto {
    pub id: u16,
    pub region_id: u16,
    pub terrain: TerrainType,

    #[serde(default, deserialize_with = "deserialize_optional")]
    pub controller: Option<String>,
    pub population: HashMap<String, u64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CountryEconomyDto {
    #[serde(default = "default_currency_code")]
    pub currency_code: String,
    #[serde(default = "default_currency_symbol")]
    pub currency_symbol: String,
    #[serde(default = "default_exchange_rate")]
    pub exchange_rate: f64,

    #[serde(default)]
    pub treasury: f64,
    #[serde(default)]
    pub national_debt: f64,
    #[serde(default)]
    pub nominal_gdp: f64,

    #[serde(default = "default_tax_rate")]
    pub corporate_tax: f64,
    #[serde(default = "default_tax_rate")]
    pub income_tax: f64,
    #[serde(default)]
    pub tariff_rate: f64,
    #[serde(default)]
    pub bailout_policy: BailoutPolicy,
    #[serde(default = "default_subsidy_budget_ratio")]
    pub max_subsidy_budget_ratio: f64,
    #[serde(default)]
    pub welfare: WelfarePolicy,
}

fn default_subsidy_budget_ratio() -> f64 {
    0.05
}

fn default_currency_code() -> String {
    "USD".to_string()
}

fn default_currency_symbol() -> String {
    "$".to_string()
}

fn default_exchange_rate() -> f64 {
    1.0
}

impl Default for CountryEconomyDto {
    fn default() -> Self {
        Self {
            currency_code: default_currency_code(),
            currency_symbol: default_currency_symbol(),
            exchange_rate: default_exchange_rate(),
            treasury: 0.0,
            national_debt: 0.0,
            nominal_gdp: 0.0,
            corporate_tax: default_tax_rate(),
            income_tax: default_tax_rate(),
            tariff_rate: 0.0,
            bailout_policy: BailoutPolicy::default(),
            max_subsidy_budget_ratio: default_subsidy_budget_ratio(),
            welfare: WelfarePolicy::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct CompanyDto {
    pub id: u32,
    pub name: String,
    pub ownership: Ownership,
    pub sector_kind: SectorKind,
    pub country_tag: String,
    pub balance: f64,
    pub reinvestment_ratio: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct StateStructureDto {
    pub kind: StructureKind,
    #[serde(default, deserialize_with = "deserialize_optional")]
    pub payer: Option<BudgetSource>,
    #[serde(default = "default_funding")]
    pub funding_level: f64,
    #[serde(default = "default_oversight")]
    pub oversight: f64,
}

#[derive(Debug, Deserialize)]
pub(super) struct RegionStructureDto {
    pub key: String,
    pub level: u16,
    #[serde(default = "default_efficiency")]
    pub efficiency: f64,
    #[serde(default = "default_wage_per_worker")]
    pub wage_per_worker: f64,
    #[serde(default = "default_budget_weight")]
    pub budget_weight: f64,
}

fn default_efficiency() -> f64 {
    1.0
}

fn default_wage_per_worker() -> f64 {
    50.0
}

fn default_budget_weight() -> f64 {
    1.0
}

fn default_funding() -> f64 {
    1.0
}

fn default_oversight() -> f64 {
    1.0
}

fn default_tax_rate() -> f64 {
    0.15
}
