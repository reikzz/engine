use crate::economy::{Resource, ResourceId};
use crate::registry::{IdentifyableDef, Registry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PopTypeId(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum NeedCategory {
    #[default]
    LifeSupport,
    Comfort,
    Luxury,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PopType {
    pub key: String,
    pub political_weight: f32,
    pub wage_multiplier: f64,
    pub is_workforce: bool,
    pub is_taxable: bool,

    #[serde(default)]
    pub needs: Vec<PopNeed>,
    #[serde(default)]
    pub science_contribution: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopSubgroup {
    pub count: u64,
    pub savings: f64,
    pub period_income: f64,
    #[serde(default = "default_needs_met")]
    pub needs_met: f32,
    #[serde(default)]
    pub debt: f64,
}

fn default_needs_met() -> f32 {
    1.0
}

impl Default for PopSubgroup {
    fn default() -> Self {
        Self {
            count: 0,
            savings: 0.0,
            period_income: 0.0,
            needs_met: 1.0,
            debt: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pop {
    pub pop_type_id: PopTypeId,
    pub employed: PopSubgroup,
    pub unemployed: PopSubgroup,
    pub daily_income: f64,
    pub average_wage: f64,
    pub satisfaction: f32,
    pub radicalism: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopNeed {
    pub resource_id: ResourceId,
    pub amount_per_capita: f64,

    #[serde(default = "default_weight")]
    pub weight: f64,

    #[serde(default)]
    pub category: NeedCategory,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PopTypeDef {
    pub key: String,
    pub political_weight: f32,
    pub wage_multiplier: f64,
    pub is_workforce: bool,
    pub is_taxable: bool,
    #[serde(default)]
    pub needs: Vec<PopNeedDef>,
    #[serde(default)]
    pub science_contribution: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PopNeedDef {
    pub resource: String,
    pub amount_per_capita: f64,
    #[serde(default = "default_weight")]
    pub weight: f64,
    #[serde(default)]
    pub category: NeedCategory,
}

fn default_weight() -> f64 {
    1.0
}

impl Pop {
    pub fn new(pop_type_id: PopTypeId, count: u64) -> Self {
        Self {
            pop_type_id,
            employed: PopSubgroup::default(),
            unemployed: PopSubgroup::new(count),
            average_wage: 0.0,
            satisfaction: 1.0,
            radicalism: 0.0,
            daily_income: 0.0,
        }
    }

    #[inline]
    pub fn count(&self) -> u64 {
        self.employed.count + self.unemployed.count
    }

    #[inline]
    pub fn free_workforce(&self) -> u64 {
        self.unemployed.count
    }

    #[inline]
    pub fn total_savings(&self) -> f64 {
        self.employed.savings + self.unemployed.savings
    }

    #[inline]
    pub fn savings_per_capita(&self) -> f64 {
        let total = self.count();
        if total == 0 {
            0.0
        } else {
            self.total_savings() / (total as f64)
        }
    }

    #[inline]
    pub fn total_debt(&self) -> f64 {
        self.employed.debt + self.unemployed.debt
    }

    #[inline]
    pub fn debt_per_capita(&self) -> f64 {
        let total = self.count();
        if total == 0 {
            0.0
        } else {
            self.total_debt() / (total as f64)
        }
    }

    pub fn hire(&mut self, count: u64) {
        Self::transfer(&mut self.unemployed, &mut self.employed, count);
    }

    pub fn fire(&mut self, count: u64) {
        Self::transfer(&mut self.employed, &mut self.unemployed, count);
    }

    fn transfer(from: &mut PopSubgroup, to: &mut PopSubgroup, count: u64) {
        if from.count == 0 || count == 0 {
            return;
        }

        let count_to_move = count.min(from.count);
        let ratio = (count_to_move as f64) / (from.count as f64);
        let capital_to_move = from.savings * ratio.min(1.0);
        let debt_to_move = from.debt * ratio.min(1.0);

        from.savings -= capital_to_move;
        from.debt -= debt_to_move;
        from.count -= count_to_move;

        let total_to_count = to.count + count_to_move;
        if total_to_count > 0 {
            to.needs_met = ((to.needs_met * to.count as f32)
                + (from.needs_met * count_to_move as f32))
                / (total_to_count as f32);
        }

        to.savings += capital_to_move;
        to.debt += debt_to_move;
        to.count += count_to_move;
    }
}

impl IdentifyableDef for PopType {
    type Id = PopTypeId;
    fn key(&self) -> &str {
        &self.key
    }
    fn from_index(index: usize) -> Self::Id {
        PopTypeId(index as u8)
    }
    fn to_index(id: Self::Id) -> usize {
        id.0 as usize
    }
}

impl PopTypeDef {
    pub fn resolve(self, resources: &Registry<ResourceId, Resource>) -> Result<PopType, String> {
        let mut resolved_needs = Vec::with_capacity(self.needs.len());

        for need in self.needs {
            let resource_id = resources.get_id(&need.resource).ok_or_else(|| {
                format!(
                    "Unknown resource '{}' in pop need for '{}'",
                    need.resource, self.key
                )
            })?;

            resolved_needs.push(PopNeed {
                resource_id,
                amount_per_capita: need.amount_per_capita,
                weight: need.weight,
                category: need.category,
            });
        }

        Ok(PopType {
            key: self.key,
            political_weight: self.political_weight,
            wage_multiplier: self.wage_multiplier,
            is_workforce: self.is_workforce,
            is_taxable: self.is_taxable,
            needs: resolved_needs,
            science_contribution: self.science_contribution,
        })
    }
}

impl PopSubgroup {
    pub fn new(count: u64) -> Self {
        Self {
            count,
            savings: 0.0,
            period_income: 0.0,
            needs_met: 1.0,
            debt: 0.0,
        }
    }
}
