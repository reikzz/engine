use super::error::LoadError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use std::fs;
use std::path::Path;

pub fn load_file<T, P>(file_path: P) -> Result<T, LoadError>
where
    T: DeserializeOwned,
    P: AsRef<Path>,
{
    let path = file_path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    ron::from_str(&content).map_err(|source| LoadError::RonParse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

pub fn load_list_from_dir<T, P>(dir_path: P, extension: &str) -> Result<Vec<T>, LoadError>
where
    T: DeserializeOwned,
    P: AsRef<Path>,
{
    let dir = dir_path.as_ref();
    if !dir.exists() {
        return Err(LoadError::DirectoryNotFound(dir.to_path_buf()));
    }

    let entries = fs::read_dir(dir).map_err(|source| LoadError::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    let mut file_paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| LoadError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some(extension) {
            file_paths.push(path);
        }
    }

    file_paths.sort();

    let mut results = Vec::new();
    for path in file_paths {
        let items: Vec<T> = load_file(&path)?;
        results.extend(items);
    }

    Ok(results)
}

pub fn deserialize_optional<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}
