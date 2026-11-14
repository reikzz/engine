use crate::economy::finance::BudgetSource;
use crate::economy::market::NationalMarket;
use crate::economy::{Resource, ResourceId};
use crate::loader::fs_utils::deserialize_optional;
use crate::registry::{IdentifyableDef, Registry};
use crate::world::map::RegionModifier;
use crate::world::{PopType, PopTypeId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StructureDefId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(usize)]
pub enum StructureKind {
    BasicEducation,
    HighEducation,
    Health,
    LawEnforcement,
    Emergency,
    Administration,
    Infrastructure,
    Financial,
}

impl StructureKind {
    #[inline]
    pub fn loc_key(&self) -> &'static str {
        match self {
            StructureKind::BasicEducation => "state_structure_basic_education",
            StructureKind::HighEducation => "state_structure_high_education",
            StructureKind::Health => "state_structure_health",
            StructureKind::LawEnforcement => "state_structure_law_enforcement",
            StructureKind::Emergency => "state_structure_emergency",
            StructureKind::Administration => "state_structure_administration",
            StructureKind::Infrastructure => "state_structure_infrastructure",
            StructureKind::Financial => "state_structure_financial",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateStructure {
    pub kind: StructureKind,
    pub payer: BudgetSource,

    pub funding_level: f64,
    pub monthly_budget: f64,
    pub oversight: f64,

    pub total_demand: f64,
    pub period_spent: f64,
    pub period_corruption: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionStructure {
    pub def_id: StructureDefId,
    pub kind: StructureKind,

    pub level: u32,
    pub wear: f64,
    pub efficiency: f64,

    pub current_workforce: Vec<(PopTypeId, u32)>,
    pub wage_per_worker: f64,

    pub budget_weight: f64,

    pub required_budget: f64,
    pub allocated_budget: f64,

    pub period_wage_cost: f64,
    pub period_maintenance_cost: f64,
}

impl RegionStructure {
    pub fn calculate_demand(
        &mut self,
        def: &StructureDef,
        pop_types: &Registry<PopTypeId, PopType>,
        market: &NationalMarket,
    ) {
        let mut wage_demand = 0.0;

        for &(pop_type_id, count_per_level) in &def.workforce_requirements {
            let total_workers_needed = count_per_level * self.level;

            let pop_def = pop_types.get(pop_type_id).expect("PopType not found");
            let pop_wage = self.wage_per_worker * pop_def.wage_multiplier;

            wage_demand += total_workers_needed as f64 * pop_wage;
        }

        let mut goods_demand = 0.0;
        for &(res_id, monthly_amount) in &def.maintenance_goods {
            let price = market.get(res_id).map(|m| m.price).unwrap_or(0.0);
            goods_demand += monthly_amount * (self.level as f64) * price;
        }

        let maintenance_demand = def.base_maintenance_per_level * (self.level as f64);

        self.required_budget = wage_demand + goods_demand + maintenance_demand;
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StructureDef<P = PopTypeId, R = ResourceId> {
    pub key: String,
    pub kind: StructureKind,

    pub workforce_requirements: Vec<(P, u32)>,

    #[serde(default)]
    pub maintenance_goods: Vec<(R, f64)>,

    pub base_maintenance_per_level: f64,
    pub capacity_per_level: u32,
    pub wear_per_month: f64,

    #[serde(default, deserialize_with = "deserialize_optional")]
    pub education: Option<EducationDef<P>>,

    #[serde(default)]
    pub modifiers: Vec<(RegionModifier, f64)>,

    #[serde(default)]
    pub construction_cost: Vec<(R, f64)>,

    #[serde(default)]
    pub build_time_days: f64,

    #[serde(default)]
    pub science_output_per_level: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EducationDef<P = PopTypeId> {
    pub candidate_pop: P,
    pub intermediate_pop: P,
    pub graduates: Vec<(P, f64)>,
    pub graduation_rate: f64,
}

pub type RawStructureDef = StructureDef<String, String>;
pub type ResolvedStructureDef = StructureDef<PopTypeId, ResourceId>;

impl IdentifyableDef for StructureDef {
    type Id = StructureDefId;

    fn key(&self) -> &str {
        &self.key
    }

    fn from_index(index: usize) -> Self::Id {
        StructureDefId(index as u16)
    }

    fn to_index(id: Self::Id) -> usize {
        id.0 as usize
    }
}

impl EducationDef<String> {
    pub fn resolve(
        self,
        structure_key: &str,
        pop_types: &Registry<PopTypeId, PopType>,
    ) -> Result<EducationDef<PopTypeId>, String> {
        let candidate_pop = pop_types.get_id(&self.candidate_pop).ok_or_else(|| {
            format!(
                "Unknown candidate PopType '{}' in education block of structure '{structure_key}'",
                self.candidate_pop
            )
        })?;

        let intermediate_pop = pop_types
            .get_id(&self.intermediate_pop)
            .ok_or_else(|| {
                format!(
                    "Unknown intermediate PopType '{}' in education block of structure '{structure_key}'",
                    self.candidate_pop
                )
            })?;

        let mut total_share = 0.0;
        let graduates = self
            .graduates
            .into_iter()
            .map(|(pop_key, share)| {
                let id = pop_types
                    .get_id(&pop_key)
                    .ok_or_else(|| {
                        format!(
                            "Unknown graduate PopType '{pop_key}' in education block of structure '{structure_key}'"
                        )
                    })?;
                total_share += share;
                Ok((id, share))
            })
            .collect::<Result<Vec<_>, String>>()?;

        if !graduates.is_empty() && (total_share - 1.0).abs() > 0.001 {
            return Err(format!(
                "Graduate shares in structure '{structure_key}' must sum to 1.0, got {total_share:.3}"
            ));
        }

        Ok(EducationDef {
            candidate_pop,
            intermediate_pop,
            graduates,
            graduation_rate: self.graduation_rate,
        })
    }
}

impl RawStructureDef {
    pub fn resolve(
        self,
        pop_types: &Registry<PopTypeId, PopType>,
        resources: &Registry<ResourceId, Resource>,
    ) -> Result<ResolvedStructureDef, String> {
        let workforce_requirements = self
            .workforce_requirements
            .into_iter()
            .map(|(pop_key, count)| {
                let id = pop_types.get_id(&pop_key).ok_or_else(|| {
                    format!("Unknown PopType '{pop_key}' in structure '{}'", self.key)
                })?;
                Ok((id, count))
            })
            .collect::<Result<Vec<_>, String>>()?;

        let maintenance_goods = self
            .maintenance_goods
            .into_iter()
            .map(|(res_key, count)| {
                let id = resources.get_id(&res_key).ok_or_else(|| {
                    format!("Unknown ResourceType '{res_key}' in structure {}", self.key)
                })?;
                Ok((id, count))
            })
            .collect::<Result<Vec<_>, String>>()?;

        let education = self
            .education
            .map(|edu| edu.resolve(&self.key, pop_types))
            .transpose()?;

        let construction_cost = self
            .construction_cost
            .into_iter()
            .map(|(res_key, count)| {
                let id = resources.get_id(&res_key).ok_or_else(|| {
                    format!("Unknown ResourceType '{res_key}' in structure {}", self.key)
                })?;
                Ok((id, count))
            })
            .collect::<Result<Vec<_>, String>>()?;

        Ok(StructureDef {
            key: self.key,
            kind: self.kind,
            workforce_requirements,
            maintenance_goods,
            base_maintenance_per_level: self.base_maintenance_per_level,
            capacity_per_level: self.capacity_per_level,
            wear_per_month: self.wear_per_month,
            education,
            modifiers: self.modifiers,
            construction_cost,
            build_time_days: self.build_time_days,
            science_output_per_level: self.science_output_per_level,
        })
    }
}
