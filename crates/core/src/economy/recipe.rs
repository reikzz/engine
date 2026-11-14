use crate::economy::{Resource, ResourceId, SectorKind};
use crate::registry::{IdentifyableDef, Registry};
use crate::world::{PopType, PopTypeId, TechnologyDef, TechnologyId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RecipeId(pub u16);

#[derive(Debug, Clone, Deserialize)]
pub struct RecipeDef {
    pub key: String,
    pub sector: SectorKind,
    pub inputs: Vec<(String, f64)>,
    pub outputs: Vec<(String, f64)>,
    pub base_expansion_cost: f64,

    pub workforce_requirements: Vec<(String, u32)>,
    pub construction_cost: Vec<(String, f64)>,
    pub build_time_days: f64,

    #[serde(default)]
    pub construction_workforce: Vec<(String, u32)>,
    #[serde(default = "default_safety_stock_days")]
    pub safety_stock_days: f64,
    #[serde(default)]
    pub required_tech: Option<String>,
}

fn default_safety_stock_days() -> f64 {
    3.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct Recipe {
    pub key: String,
    pub sector: SectorKind,
    pub inputs: Vec<(ResourceId, f64)>,
    pub outputs: Vec<(ResourceId, f64)>,
    pub base_expansion_cost: f64,

    pub workforce_requirements: Vec<(PopTypeId, u32)>,
    pub construction_cost: Vec<(ResourceId, f64)>,
    pub build_time_days: f64,

    pub construction_workforce: Vec<(PopTypeId, u32)>,
    pub safety_stock_days: f64,
    pub required_tech: Option<TechnologyId>,
}

impl IdentifyableDef for Recipe {
    type Id = RecipeId;

    fn key(&self) -> &str {
        &self.key
    }
    fn from_index(index: usize) -> Self::Id {
        RecipeId(index as u16)
    }
    fn to_index(id: Self::Id) -> usize {
        id.0 as usize
    }
}

impl RecipeDef {
    pub fn resolve(
        self,
        pop_types: &Registry<PopTypeId, PopType>,
        resources: &Registry<ResourceId, Resource>,
        technologies: &Registry<TechnologyId, TechnologyDef>,
    ) -> Result<Recipe, String> {
        let required_tech = match self.required_tech.as_deref() {
            Some(tech_key) => {
                let id = technologies.get_id(tech_key).ok_or_else(|| {
                    format!(
                        "Unknown required technology '{}' in recipe '{}'",
                        tech_key, self.key
                    )
                })?;
                Some(id)
            }
            None => None,
        };
        let workforce_requirements = self
            .workforce_requirements
            .into_iter()
            .map(|(pop_key, count)| {
                let id = pop_types.get_id(&pop_key).ok_or_else(|| {
                    format!("Unknown PopType key '{pop_key}' in recipe '{}'", self.key)
                })?;
                Ok((id, count))
            })
            .collect::<Result<Vec<_>, String>>()?;

        let inputs = self
            .inputs
            .into_iter()
            .map(|(input, amount)| {
                let id = resources.get_id(&input).ok_or_else(|| {
                    format!(
                        "Unknown input Resource id '{input}' in recipe '{}'",
                        self.key
                    )
                })?;
                Ok((id, amount))
            })
            .collect::<Result<Vec<_>, String>>()?;

        let outputs = self
            .outputs
            .into_iter()
            .map(|(output, amount)| {
                let id = resources.get_id(&output).ok_or_else(|| {
                    format!(
                        "Unknown output Resource id '{output}' in recipe '{}'",
                        self.key
                    )
                })?;
                Ok((id, amount))
            })
            .collect::<Result<Vec<_>, String>>()?;

        let construction_cost = self
            .construction_cost
            .into_iter()
            .map(|(output, amount)| {
                let id = resources.get_id(&output).ok_or_else(|| {
                    format!(
                        "Unknown construction cost Resource id '{output}' in recipe '{}'",
                        self.key
                    )
                })?;
                Ok((id, amount))
            })
            .collect::<Result<Vec<_>, String>>()?;

        let construction_workforce = self
            .construction_workforce
            .into_iter()
            .map(|(pop_key, count)| {
                let id = pop_types.get_id(&pop_key).ok_or_else(|| {
                    format!("Unknown PopType key '{pop_key}' in recipe '{}'", self.key)
                })?;
                Ok((id, count))
            })
            .collect::<Result<Vec<_>, String>>()?;

        let safety_stock_days = if self.safety_stock_days > 0.0 {
            self.safety_stock_days
        } else {
            3.0
        };

        Ok(Recipe {
            key: self.key,
            sector: self.sector,
            inputs,
            outputs,
            base_expansion_cost: self.base_expansion_cost,
            workforce_requirements,
            construction_cost,
            build_time_days: self.build_time_days,
            construction_workforce,
            safety_stock_days,
            required_tech,
        })
    }
}
