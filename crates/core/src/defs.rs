use crate::economy::{Recipe, RecipeId, Resource, ResourceId, Sector, SectorKind};
use crate::registry::Registry;
use crate::world::{PopType, PopTypeId, StructureDef, StructureDefId, TechnologyDef, TechnologyId};

#[derive(Debug, Default)]
pub struct GameDefs {
    pub resources: Registry<ResourceId, Resource>,
    pub sectors: Registry<SectorKind, Sector>,
    pub recipes: Registry<RecipeId, Recipe>,
    pub pop_types: Registry<PopTypeId, PopType>,
    pub structures: Registry<StructureDefId, StructureDef>,
    pub technologies: Registry<TechnologyId, TechnologyDef>,
}

impl GameDefs {
    pub fn new() -> Self {
        Self::default()
    }
}
