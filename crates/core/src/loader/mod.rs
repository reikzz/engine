pub mod defs_loader;
mod dto;
pub mod error;
pub mod fs_utils;
pub mod world_loader;

use self::error::LoadError;
use crate::World;
use crate::defs::GameDefs;
use serde::de::DeserializeOwned;
use std::path::Path;

pub struct DataLoader;

impl DataLoader {
    pub fn load_list_from_dir<T, P>(dir_path: P, extension: &str) -> Result<Vec<T>, LoadError>
    where
        T: DeserializeOwned,
        P: AsRef<Path>,
    {
        fs_utils::load_list_from_dir(dir_path, extension)
    }

    pub fn load_file<T, P>(file_path: P) -> Result<T, LoadError>
    where
        T: DeserializeOwned,
        P: AsRef<Path>,
    {
        fs_utils::load_file(file_path)
    }

    pub fn load_game_defs<P: AsRef<Path>>(defs_dir: P) -> Result<GameDefs, LoadError> {
        defs_loader::load_game_defs(defs_dir)
    }

    pub fn load_world<P: AsRef<Path>>(scenario_dir: P, defs: GameDefs) -> Result<World, LoadError> {
        world_loader::load_world(scenario_dir, defs)
    }
}
