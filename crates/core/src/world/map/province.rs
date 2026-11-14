use crate::world::country::CountryTag;
use crate::world::map::RegionId;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProvinceId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum TerrainType {
    Plains,
    Forest,
    Hills,
    Mountains,
    City,
    Desert,
    Marsh,
}

impl TerrainType {
    #[inline]
    pub fn base_infrastructure_capacity(self) -> f64 {
        match self {
            Self::Plains => 50.0,
            Self::City => 60.0,
            Self::Hills => 35.0,
            Self::Forest => 30.0,
            Self::Marsh => 20.0,
            Self::Mountains => 15.0,
            Self::Desert => 15.0,
        }
    }
}

pub struct Province {
    pub id: ProvinceId,
    pub region_id: RegionId,

    pub terrain: TerrainType,

    pub controller: Option<CountryTag>,

    pub population_weight: f64,
}

impl Province {
    pub fn new(
        id: ProvinceId,
        region_id: RegionId,
        terrain: TerrainType,
        controller: Option<CountryTag>,
        population_weight: f64,
    ) -> Self {
        Self {
            id,
            region_id,
            terrain,
            controller,
            population_weight,
        }
    }
}
