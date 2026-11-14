use crate::registry::IdentifyableDef;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TechnologyId(pub u16);

#[derive(Debug, Clone, Deserialize)]
pub struct RawTechnologyDef {
    pub key: String,
    pub name: String,
    pub cost: f64,
    #[serde(default)]
    pub prerequisites: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnologyDef {
    pub key: String,
    pub name: String,
    pub cost: f64,
    pub prerequisites: Vec<TechnologyId>,
}

impl IdentifyableDef for TechnologyDef {
    type Id = TechnologyId;

    fn key(&self) -> &str {
        &self.key
    }

    fn from_index(index: usize) -> Self::Id {
        TechnologyId(index as u16)
    }

    fn to_index(id: Self::Id) -> usize {
        id.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TechnologyResearchStatus {
    Researched,
    Active,
    Available,
    Locked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TechnologyOverviewItem {
    pub id: TechnologyId,
    pub key: String,
    pub name: String,
    pub cost: f64,
    pub prerequisites: Vec<TechnologyId>,
    pub status: TechnologyResearchStatus,
    pub progress: f64,
    pub progress_ratio: f64,
}
