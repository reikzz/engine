use crate::map::geo_loader;
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct MapVertex {
    pub position: [f32; 2],
    pub province_id: u32,
    pub is_border: u32,
    pub edge_distance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvinceVisualDef {
    pub id: u16,
    pub country_tag: String,
    pub name: String,
    pub points: Vec<[f32; 2]>,
    #[serde(default)]
    pub bbox: [f32; 4],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MapDataDef {
    pub provinces: Vec<ProvinceVisualDef>,
    #[serde(default, skip_serializing)]
    pub suppressed_border_edges: HashSet<((i32, i32), (i32, i32))>,
}

pub struct MapMesh {
    pub vertices: Vec<MapVertex>,
    pub fill_indices: Vec<u32>,
    pub border_indices: Vec<u32>,
    pub province_centers: BTreeMap<u16, Vec2>,
}

fn calculate_miter_offset(n0: Vec2, n1: Vec2) -> Vec2 {
    let sum = n0 + n1;
    let denom = 1.0 + n0.dot(n1);
    if denom > 0.3 {
        let miter = sum / denom;
        if miter.length_squared() > 2.25 {
            miter.normalize() * 1.5
        } else {
            miter
        }
    } else if n1.length_squared() > 1e-4 {
        n1
    } else {
        Vec2::Y
    }
}

fn segment_direction_and_normal(p0: Vec2, p1: Vec2) -> (Vec2, Vec2) {
    let dir = (p1 - p0).normalize_or_zero();
    let normal = if dir.length_squared() > 1e-5 {
        Vec2::new(-dir.y, dir.x)
    } else {
        Vec2::Y
    };
    (dir, normal)
}

fn quantize_spatial_key(p0: [f32; 2], p1: [f32; 2]) -> ((i32, i32), (i32, i32)) {
    let k0 = ((p0[0] * 2.0).round() as i32, (p0[1] * 2.0).round() as i32);
    let k1 = ((p1[0] * 2.0).round() as i32, (p1[1] * 2.0).round() as i32);
    if k0 < k1 { (k0, k1) } else { (k1, k0) }
}

impl MapDataDef {
    pub fn load_or_generate(path: &Path, split_countries: &[&str]) -> Self {
        let candidate_paths = [
            path.to_path_buf(),
            PathBuf::from("../../").join(path),
            PathBuf::from("../").join(path),
        ];

        for candidate in &candidate_paths {
            if candidate.exists() {
                if let Ok(content) = fs::read_to_string(candidate) {
                    let clean = content.trim_start_matches('\u{feff}');
                    if let Ok(def) = ron::from_str::<MapDataDef>(clean) {
                        if !def.provinces.is_empty() {
                            return def;
                        }
                    }
                }
            }
        }

        if let Some(def) = geo_loader::load_world_map(
            Path::new("data/map/world_subdivisions.geojson"),
            Path::new("data/map/countries.geojson"),
            split_countries,
        ) {
            return def;
        }

        panic!("No provinces definition found")
    }

    pub fn build_mesh(&self) -> MapMesh {
        let mut vertices = Vec::new();
        let mut fill_indices = Vec::new();
        let mut halo_indices = Vec::new();
        let mut regional_border_indices = Vec::new();
        let mut coastline_border_indices = Vec::new();
        let mut national_border_indices = Vec::new();
        let mut province_centers = BTreeMap::new();
        let mut seen_border_edges = HashSet::new();

        for prov in &self.provinces {
            let num_points = prov.points.len();
            if num_points < 3 {
                continue;
            }

            let mut center_acc = Vec2::ZERO;
            for p in &prov.points {
                center_acc += Vec2::new(p[0], p[1]);
            }
            let center = center_acc / (num_points as f32);
            province_centers.insert(prov.id, center);

            let base_vertex_idx = vertices.len() as u32;

            let mut flat_coords = Vec::with_capacity(num_points * 2);
            for p in &prov.points {
                let pos = Vec2::new(p[0], p[1]);
                flat_coords.push(p[0] as f64);
                flat_coords.push(p[1] as f64);

                let dist = (pos - center).length();
                let norm_dist = (dist / 100.0).clamp(0.3, 1.0);

                vertices.push(MapVertex {
                    position: *p,
                    province_id: prov.id as u32,
                    is_border: 0,
                    edge_distance: norm_dist,
                });
            }

            if let Ok(triangles) = earcutr::earcut(&flat_coords, &[], 2) {
                for tri_idx in triangles {
                    fill_indices.push(base_vertex_idx + tri_idx as u32);
                }
            }
        }

        let mut country_prov_counts: HashMap<&str, HashSet<u16>> = HashMap::new();
        for prov in &self.provinces {
            country_prov_counts
                .entry(prov.country_tag.as_str())
                .or_default()
                .insert(prov.id);
        }
        let countries_with_regions: HashSet<&str> = country_prov_counts
            .into_iter()
            .filter(|(_, prov_ids)| prov_ids.len() > 1)
            .map(|(tag, _)| tag)
            .collect();

        let mut edge_to_countries: HashMap<((i32, i32), (i32, i32)), Vec<&str>> = HashMap::new();

        for prov in &self.provinces {
            let num_points = prov.points.len();
            if num_points < 3 {
                continue;
            }
            let group = prov.country_tag.as_str();
            for i in 0..num_points {
                let next_i = (i + 1) % num_points;
                let spatial_key = quantize_spatial_key(prov.points[i], prov.points[next_i]);
                if spatial_key.0 == spatial_key.1 {
                    continue;
                }
                edge_to_countries
                    .entry(spatial_key)
                    .or_default()
                    .push(group);
            }
        }

        for prov in &self.provinces {
            let num_points = prov.points.len();
            if num_points < 3 {
                continue;
            }
            let group = prov.country_tag.as_str();
            let has_regions = countries_with_regions.contains(group);

            let mut segment_dirs = Vec::with_capacity(num_points);
            let mut segment_normals = Vec::with_capacity(num_points);
            let mut segment_types = Vec::with_capacity(num_points);

            for i in 0..num_points {
                let next_i = (i + 1) % num_points;
                let p0 = Vec2::new(prov.points[i][0], prov.points[i][1]);
                let p1 = Vec2::new(prov.points[next_i][0], prov.points[next_i][1]);
                let (dir, normal) = segment_direction_and_normal(p0, p1);
                segment_dirs.push(dir);
                segment_normals.push(normal);

                let spatial_key = quantize_spatial_key(prov.points[i], prov.points[next_i]);
                let b_type = if spatial_key.0 == spatial_key.1 {
                    1u32
                } else if let Some(countries) = edge_to_countries.get(&spatial_key) {
                    if countries.len() == 1 {
                        5u32
                    } else if countries.iter().all(|&c| c == group) {
                        2u32
                    } else {
                        1u32
                    }
                } else {
                    1u32
                };
                segment_types.push(b_type);
            }

            for i in 0..num_points {
                let prev_i = (i + num_points - 1) % num_points;
                let next_i = (i + 1) % num_points;

                let p0 = Vec2::new(prov.points[i][0], prov.points[i][1]);
                let p1 = Vec2::new(prov.points[next_i][0], prov.points[next_i][1]);
                let seg_len = (p1 - p0).length();
                if seg_len < 0.1 {
                    continue;
                }

                let spatial_key = quantize_spatial_key(prov.points[i], prov.points[next_i]);
                if spatial_key.0 == spatial_key.1 {
                    continue;
                }

                let border_type = segment_types[i];
                let is_national = border_type == 1 || border_type == 5;

                if is_national
                    && !has_regions
                    && self.suppressed_border_edges.contains(&spatial_key)
                {
                    continue;
                }

                if is_national {
                    let inset_width = 5.0f32.min(seg_len * 0.40);
                    let halo_base = vertices.len() as u32;

                    let miter_start =
                        calculate_miter_offset(segment_normals[prev_i], segment_normals[i]);
                    let miter_end =
                        calculate_miter_offset(segment_normals[i], segment_normals[next_i]);

                    vertices.push(MapVertex {
                        position: p0.into(),
                        province_id: prov.id as u32,
                        is_border: 3,
                        edge_distance: 1.0,
                    });
                    vertices.push(MapVertex {
                        position: (p0 + miter_start * inset_width).into(),
                        province_id: prov.id as u32,
                        is_border: 3,
                        edge_distance: 0.0,
                    });
                    vertices.push(MapVertex {
                        position: p1.into(),
                        province_id: prov.id as u32,
                        is_border: 3,
                        edge_distance: 1.0,
                    });
                    vertices.push(MapVertex {
                        position: (p1 + miter_end * inset_width).into(),
                        province_id: prov.id as u32,
                        is_border: 3,
                        edge_distance: 0.0,
                    });

                    halo_indices.extend_from_slice(&[
                        halo_base,
                        halo_base + 1,
                        halo_base + 2,
                        halo_base + 1,
                        halo_base + 3,
                        halo_base + 2,
                    ]);
                }

                if !seen_border_edges.insert(spatial_key) {
                    continue;
                }

                let is_prev_same = segment_types[prev_i] == border_type;
                let is_next_same = segment_types[next_i] == border_type;

                let dir = segment_dirs[i];
                let normal = segment_normals[i];

                let width = if border_type == 1 {
                    1.8f32
                } else if border_type == 5 {
                    1.2f32
                } else {
                    0.9f32
                };

                let mut start_pos = p0;
                let mut end_pos = p1;

                if border_type == 2 {
                    let junction_retract = 1.6f32.min(seg_len * 0.45);
                    if !is_prev_same {
                        start_pos += dir * junction_retract;
                    }
                    if !is_next_same {
                        end_pos -= dir * junction_retract;
                    }
                }

                let miter_start = if is_prev_same {
                    calculate_miter_offset(segment_normals[prev_i], normal)
                } else {
                    normal
                };

                let miter_end = if is_next_same {
                    calculate_miter_offset(normal, segment_normals[next_i])
                } else {
                    normal
                };

                let border_base = vertices.len() as u32;
                vertices.push(MapVertex {
                    position: (start_pos - miter_start * width).into(),
                    province_id: prov.id as u32,
                    is_border: border_type,
                    edge_distance: 1.0,
                });
                vertices.push(MapVertex {
                    position: (start_pos + miter_start * width).into(),
                    province_id: prov.id as u32,
                    is_border: border_type,
                    edge_distance: 1.0,
                });
                vertices.push(MapVertex {
                    position: (end_pos - miter_end * width).into(),
                    province_id: prov.id as u32,
                    is_border: border_type,
                    edge_distance: 1.0,
                });
                vertices.push(MapVertex {
                    position: (end_pos + miter_end * width).into(),
                    province_id: prov.id as u32,
                    is_border: border_type,
                    edge_distance: 1.0,
                });

                let quad = [
                    border_base,
                    border_base + 1,
                    border_base + 2,
                    border_base + 1,
                    border_base + 3,
                    border_base + 2,
                ];

                if border_type == 1 {
                    national_border_indices.extend_from_slice(&quad);
                } else if border_type == 5 {
                    coastline_border_indices.extend_from_slice(&quad);
                } else {
                    regional_border_indices.extend_from_slice(&quad);
                }
            }
        }

        let mut border_indices = Vec::with_capacity(
            halo_indices.len()
                + regional_border_indices.len()
                + coastline_border_indices.len()
                + national_border_indices.len(),
        );
        border_indices.extend(halo_indices);
        border_indices.extend(regional_border_indices);
        border_indices.extend(coastline_border_indices);
        border_indices.extend(national_border_indices);

        MapMesh {
            vertices,
            fill_indices,
            border_indices,
            province_centers,
        }
    }
}

pub fn build_selection_outline_mesh(
    provinces: &[ProvinceVisualDef],
    selected_id: u16,
) -> (Vec<MapVertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for prov in provinces {
        if prov.id != selected_id {
            continue;
        }

        let num_points = prov.points.len();
        if num_points < 3 {
            continue;
        }

        let mut segment_dirs = Vec::with_capacity(num_points);
        let mut segment_normals = Vec::with_capacity(num_points);

        for i in 0..num_points {
            let next_i = (i + 1) % num_points;
            let p0 = Vec2::new(prov.points[i][0], prov.points[i][1]);
            let p1 = Vec2::new(prov.points[next_i][0], prov.points[next_i][1]);
            let (dir, normal) = segment_direction_and_normal(p0, p1);
            segment_dirs.push(dir);
            segment_normals.push(normal);
        }

        let width = 2.4f32;

        for i in 0..num_points {
            let prev_i = (i + num_points - 1) % num_points;
            let next_i = (i + 1) % num_points;

            let p0 = Vec2::new(prov.points[i][0], prov.points[i][1]);
            let p1 = Vec2::new(prov.points[next_i][0], prov.points[next_i][1]);
            let seg_len = (p1 - p0).length();
            if seg_len < 0.1 {
                continue;
            }

            let miter_start = calculate_miter_offset(segment_normals[prev_i], segment_normals[i]);
            let miter_end = calculate_miter_offset(segment_normals[i], segment_normals[next_i]);

            let base = vertices.len() as u32;
            vertices.push(MapVertex {
                position: (p0 - miter_start * width).into(),
                province_id: selected_id as u32,
                is_border: 4,
                edge_distance: 1.0,
            });
            vertices.push(MapVertex {
                position: (p0 + miter_start * width).into(),
                province_id: selected_id as u32,
                is_border: 4,
                edge_distance: 1.0,
            });
            vertices.push(MapVertex {
                position: (p1 - miter_end * width).into(),
                province_id: selected_id as u32,
                is_border: 4,
                edge_distance: 1.0,
            });
            vertices.push(MapVertex {
                position: (p1 + miter_end * width).into(),
                province_id: selected_id as u32,
                is_border: 4,
                edge_distance: 1.0,
            });

            indices.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
        }
    }

    (vertices, indices)
}
