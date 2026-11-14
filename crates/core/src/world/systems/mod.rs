mod construction;
mod demography;
mod education;
pub mod logistics;
mod structure;
mod technology;

use crate::system::SystemPipeline;
use crate::world::systems::construction::ConstructionSystem;
use crate::world::systems::demography::DemographySystem;
use crate::world::systems::education::EducationSystem;
pub use crate::world::systems::logistics::LogisticsSystem;
use crate::world::systems::structure::StructureSystem;
use crate::world::systems::technology::TechnologySystem;

pub fn register_systems(pipeline: &mut SystemPipeline) {
    pipeline.add_system(ConstructionSystem);
    pipeline.add_system(StructureSystem::default());
    pipeline.add_system(EducationSystem);
    pipeline.add_system(DemographySystem::default());
    pipeline.add_system(TechnologySystem);
}
