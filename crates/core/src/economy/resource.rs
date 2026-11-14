use crate::registry::IdentifyableDef;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResourceId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum ResourceCategory {
    Energy,
    RawMaterial,
    Intermediate,
    HighTech,
    EndProduct,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Resource {
    pub key: String,
    pub category: ResourceCategory,
    pub base_price: f64,

    #[serde(default = "default_storable")]
    pub is_storable: bool,

    #[serde(default)]
    pub spoilage_rate: Option<f64>,

    #[serde(default)]
    pub requires_special_transport: bool,
    pub weight: f64,
}

impl IdentifyableDef for Resource {
    type Id = ResourceId;

    fn key(&self) -> &str {
        &self.key
    }

    fn from_index(index: usize) -> Self::Id {
        ResourceId(index as u16)
    }

    fn to_index(id: Self::Id) -> usize {
        id.0 as usize
    }
}

impl ResourceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into().parse().unwrap())
    }
}

impl From<usize> for ResourceId {
    fn from(u: usize) -> Self {
        Self(u.try_into().unwrap())
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Res#{}", self.0)
    }
}

impl ResourceId {
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

fn default_storable() -> bool {
    true
}
