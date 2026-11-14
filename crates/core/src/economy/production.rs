use crate::economy::{RecipeId, ResourceId};
use crate::world::PopTypeId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProductionLineId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionLineState {
    pub id: ProductionLineId,
    pub recipe_id: RecipeId,
    pub installed_capacity: u32,
    pub natural_max_capacity: Option<u32>,
    pub target_capacity_cap: Option<u32>,
    pub utilization: f64,
    pub potential_utilization: f64,
    pub material_utilization: f64,
    pub priority: Priority,
    pub employed_workers: Vec<(PopTypeId, u32)>,
    pub target_wage: f64,
    pub expansion_progress: f64,

    #[serde(default)]
    pub last_day_output: f64,
    #[serde(default)]
    pub input_shortage: Vec<(ResourceId, f64)>,
    #[serde(default)]
    pub consecutive_idle_days: u32,
}

impl ProductionLineState {
    #[inline]
    pub fn get_employed(&self, pop_type: PopTypeId) -> u32 {
        self.employed_workers
            .iter()
            .find(|(id, _)| *id == pop_type)
            .map(|(_, count)| *count)
            .unwrap_or(0)
    }

    #[inline]
    pub fn add_employed(&mut self, pop_type: PopTypeId, count: u32) {
        if let Some((_, current)) = self
            .employed_workers
            .iter_mut()
            .find(|(id, _)| *id == pop_type)
        {
            *current += count;
        } else {
            self.employed_workers.push((pop_type, count));
        }
    }

    #[inline]
    pub fn set_employed(&mut self, pop_type: PopTypeId, count: u32) {
        if let Some((_, current)) = self
            .employed_workers
            .iter_mut()
            .find(|(id, _)| *id == pop_type)
        {
            *current = count;
        } else {
            self.employed_workers.push((pop_type, count));
        }
    }
}
