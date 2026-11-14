use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_POLITICAL_PALETTE: [[f32; 4]; 32] = [
    [0.35, 0.55, 0.42, 1.0],
    [0.65, 0.48, 0.35, 1.0],
    [0.45, 0.40, 0.60, 1.0],
    [0.32, 0.58, 0.58, 1.0],
    [0.58, 0.38, 0.45, 1.0],
    [0.52, 0.55, 0.32, 1.0],
    [0.40, 0.48, 0.62, 1.0],
    [0.62, 0.55, 0.38, 1.0],
    [0.38, 0.52, 0.48, 1.0],
    [0.55, 0.42, 0.52, 1.0],
    [0.48, 0.58, 0.38, 1.0],
    [0.60, 0.45, 0.40, 1.0],
    [0.36, 0.44, 0.58, 1.0],
    [0.56, 0.50, 0.34, 1.0],
    [0.42, 0.54, 0.50, 1.0],
    [0.50, 0.40, 0.54, 1.0],
    [0.44, 0.58, 0.46, 1.0],
    [0.60, 0.40, 0.36, 1.0],
    [0.38, 0.50, 0.64, 1.0],
    [0.64, 0.58, 0.36, 1.0],
    [0.48, 0.36, 0.56, 1.0],
    [0.34, 0.56, 0.52, 1.0],
    [0.58, 0.48, 0.38, 1.0],
    [0.42, 0.44, 0.60, 1.0],
    [0.54, 0.58, 0.42, 1.0],
    [0.62, 0.38, 0.48, 1.0],
    [0.36, 0.52, 0.58, 1.0],
    [0.52, 0.44, 0.38, 1.0],
    [0.40, 0.56, 0.44, 1.0],
    [0.58, 0.42, 0.56, 1.0],
    [0.46, 0.52, 0.62, 1.0],
    [0.62, 0.50, 0.42, 1.0],
];

#[derive(Debug, Clone, PartialEq)]
pub struct CountryLabelAnchor {
    pub name: String,
    pub center: glam::Vec2,
    pub world_width: f32,
    pub world_height: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CountryNamesDef {
    #[serde(default)]
    pub names: BTreeMap<String, String>,
    #[serde(default)]
    pub colors: BTreeMap<String, (f32, f32, f32)>,
    #[serde(default)]
    pub anchors: BTreeMap<String, (f32, f32)>,
}

impl CountryNamesDef {
    pub fn load(path: &Path) -> Self {
        let candidate_paths = [
            path.to_path_buf(),
            PathBuf::from("../../").join(path),
            PathBuf::from("../").join(path),
        ];

        for candidate in &candidate_paths {
            if candidate.exists() {
                if let Ok(content) = fs::read_to_string(candidate) {
                    let clean = content.trim_start_matches('\u{feff}');
                    if let Ok(def) = ron::from_str::<CountryNamesDef>(clean) {
                        return def;
                    }
                }
            }
        }

        Self::default()
    }

    pub fn resolve_display_name<'a>(&'a self, key: &'a str) -> &'a str {
        if let Some(name) = self.names.get(key) {
            name.as_str()
        } else {
            key
        }
    }

    pub fn resolve_anchor(&self, key: &str) -> Option<(f32, f32)> {
        self.anchors.get(key).copied()
    }

    pub fn resolve_color(&self, key: &str, fallback_index: usize) -> [f32; 4] {
        if let Some(&(r, g, b)) = self.colors.get(key) {
            [r, g, b, 1.0]
        } else {
            let mut hash: usize = 0;
            for b in key.as_bytes() {
                hash = hash.wrapping_mul(31).wrapping_add(*b as usize);
            }
            let idx = (hash + fallback_index) % DEFAULT_POLITICAL_PALETTE.len();
            DEFAULT_POLITICAL_PALETTE[idx]
        }
    }
}
