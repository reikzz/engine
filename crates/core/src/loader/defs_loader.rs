use crate::defs::GameDefs;
use crate::economy::{RecipeDef, Resource, Sector};
use crate::loader::error::LoadError;
use crate::loader::fs_utils::load_list_from_dir;
use crate::world::{PopTypeDef, RawStructureDef, RawTechnologyDef, TechnologyDef, TechnologyId};
use std::collections::HashMap;
use std::path::Path;

pub fn load_game_defs<P: AsRef<Path>>(defs_dir: P) -> Result<GameDefs, LoadError> {
    let p = defs_dir.as_ref();
    let mut defs = GameDefs::new();

    for res in load_list_from_dir::<Resource, _>(p.join("resources"), "ron")? {
        defs.resources
            .register(res)
            .map_err(|e| LoadError::Registration(format!("Resource: {}", e)))?;
    }

    if p.join("technologies").exists() {
        let raw_technologies =
            load_list_from_dir::<RawTechnologyDef, _>(p.join("technologies"), "ron")?;
        let mut tech_key_map = HashMap::with_capacity(raw_technologies.len());
        for (idx, raw_tech) in raw_technologies.iter().enumerate() {
            tech_key_map.insert(raw_tech.key.clone(), TechnologyId(idx as u16));
        }

        for raw_tech in raw_technologies {
            let mut prereq_ids = Vec::with_capacity(raw_tech.prerequisites.len());
            for prereq_key in &raw_tech.prerequisites {
                let prereq_id = tech_key_map.get(prereq_key).copied().ok_or_else(|| {
                    LoadError::Registration(format!(
                        "Prerequisite '{}' for tech '{}' not found",
                        prereq_key, raw_tech.key
                    ))
                })?;
                prereq_ids.push(prereq_id);
            }

            let tech_def = TechnologyDef {
                key: raw_tech.key,
                name: raw_tech.name,
                cost: raw_tech.cost.max(1.0),
                prerequisites: prereq_ids,
            };

            defs.technologies
                .register(tech_def)
                .map_err(|e| LoadError::Registration(format!("Technology: {}", e)))?;
        }
    }

    let raw_pops = load_list_from_dir::<PopTypeDef, _>(p.join("pops"), "ron")?;
    for raw_pop in raw_pops {
        let pop = raw_pop
            .resolve(&defs.resources)
            .map_err(|e| LoadError::Registration(format!("Pop Type resolve error: {}", e)))?;

        defs.pop_types
            .register(pop)
            .map_err(|e| LoadError::Registration(format!("Pop Type: {}", e)))?;
    }

    let raw_recipes = load_list_from_dir::<RecipeDef, _>(p.join("recipes"), "ron")?;
    for raw_rec in raw_recipes {
        let recipe = raw_rec
            .resolve(&defs.pop_types, &defs.resources, &defs.technologies)
            .map_err(|e| LoadError::Registration(format!("Recipe resolve error: {}", e)))?;

        defs.recipes
            .register(recipe)
            .map_err(|e| LoadError::Registration(format!("Recipe: {}", e)))?;
    }

    for sec in load_list_from_dir::<Sector, _>(p.join("sectors"), "ron")? {
        defs.sectors
            .register(sec)
            .map_err(|e| LoadError::Registration(format!("Sector: {}", e)))?;
    }

    let raw_structures = load_list_from_dir::<RawStructureDef, _>(p.join("structures"), "ron")?;
    for raw_struct in raw_structures {
        let structure = raw_struct
            .resolve(&defs.pop_types, &defs.resources)
            .map_err(|e| LoadError::Registration(format!("Structure resolve error: {}", e)))?;

        defs.structures
            .register(structure)
            .map_err(|e| LoadError::Registration(format!("Structure: {}", e)))?;
    }

    Ok(defs)
}
