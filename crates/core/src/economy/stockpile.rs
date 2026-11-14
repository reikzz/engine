use crate::economy::ResourceId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stockpile {
    amounts: Vec<f64>,
}

impl Stockpile {
    pub fn new(resource_count: usize) -> Self {
        Self {
            amounts: vec![0.0; resource_count],
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (ResourceId, f64)> + '_ {
        self.amounts
            .iter()
            .enumerate()
            .map(|(index, &amount)| (ResourceId::from(index), amount))
    }

    pub fn non_empty(&self) -> impl Iterator<Item = (ResourceId, f64)> + '_ {
        self.amounts
            .iter()
            .enumerate()
            .filter(|&(_, &amount)| amount > 0.0)
            .map(|(index, &amount)| (ResourceId::from(index), amount))
    }

    #[inline]
    pub fn get(&self, id: ResourceId) -> f64 {
        self.amounts.get(id.index()).copied().unwrap_or(0.0)
    }

    #[inline]
    pub fn add(&mut self, id: ResourceId, delta: f64) {
        if let Some(slot) = self.amounts.get_mut(id.index()) {
            *slot += delta;
        }
    }

    pub fn try_consume(&mut self, id: ResourceId, amount: f64) -> bool {
        if amount <= 0.0 {
            return true;
        }

        if let Some(slot) = self.amounts.get_mut(id.index())
            && *slot >= amount
        {
            *slot -= amount;
            return true;
        }
        false
    }
}
