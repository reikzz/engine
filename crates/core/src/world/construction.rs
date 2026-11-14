use crate::economy::finance::BudgetSource;
use crate::economy::{CompanyId, RealEstateKind, RecipeId, ResourceId, SectorKind};
use crate::world::map::RegionId;
use crate::world::{PopTypeId, StructureDefId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ConstructionTarget {
    NewFacility {
        company_id: CompanyId,
        recipe_id: RecipeId,
        region_id: RegionId,
        sector_kind: SectorKind,
    },
    UpgradeStructure {
        region_id: RegionId,
        structure_id: StructureDefId,
        def_id: StructureDefId,
    },
    NewRealEstate {
        company_id: Option<CompanyId>,
        kind: RealEstateKind,
        region_id: RegionId,
        units: u32,
        base_rent: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstructionMaterialEstimate {
    pub resource_id: ResourceId,
    pub total_quantity: f64,
    pub daily_quantity: f64,
    pub local_unit_price: f64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FacilityConstructionEstimate {
    pub recipe_id: RecipeId,
    pub build_days: f64,
    pub total_materials_cost: f64,
    pub materials: Vec<ConstructionMaterialEstimate>,
    pub daily_workforce_wage: f64,
    pub total_expected_wages: f64,
    pub contractor_profit: f64,
    pub contractor_fee: f64,
    pub daily_contractor_fee: f64,
    pub mobilization_advance: f64,
    pub total_estimated_cost: f64,
    pub is_tech_unlocked: bool,
    pub can_afford_advance: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureUpgradeEstimate {
    pub structure_def_id: StructureDefId,
    pub build_days: f64,
    pub total_materials_cost: f64,
    pub materials: Vec<ConstructionMaterialEstimate>,
    pub daily_workforce_wage: f64,
    pub total_expected_wages: f64,
    pub contractor_profit: f64,
    pub contractor_fee: f64,
    pub daily_contractor_fee: f64,
    pub mobilization_advance: f64,
    pub total_estimated_cost: f64,
    pub can_afford_advance: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstructionProject {
    pub target: ConstructionTarget,
    pub contractor: CompanyId,
    pub daily_contractor_fee: f64,
    pub payer: BudgetSource,
    pub progress: f64,
    pub total_days: f64,
    pub daily_materials: Vec<(ResourceId, f64)>,
    pub current_workforce: Vec<(PopTypeId, u32)>,
    pub required_workers: Vec<(PopTypeId, u32)>,
    pub last_effective_speed: f64,
    pub consecutive_stalled_days: u16,
}
