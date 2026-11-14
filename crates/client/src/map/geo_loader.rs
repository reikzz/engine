use crate::map::geometry::{MapDataDef, ProvinceVisualDef};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Default)]
struct ProvinceMapping {
    #[serde(default)]
    countries: BTreeMap<String, u16>,
    #[serde(default)]
    subdivisions: BTreeMap<String, u16>,
}

#[derive(Deserialize)]
struct GeoJsonRoot {
    features: Vec<GeoJsonFeature>,
}

#[derive(Deserialize)]
struct GeoJsonFeature {
    properties: serde_json::Map<String, serde_json::Value>,
    geometry: GeoJsonGeometry,
}

#[derive(Deserialize)]
struct GeoJsonGeometry {
    #[serde(rename = "type")]
    geom_type: String,
    coordinates: serde_json::Value,
}

pub fn load_world_map(
    subdivisions_path: &Path,
    countries_path: &Path,
    split_countries: &[&str],
) -> Option<MapDataDef> {
    let sub_path = resolve_existing_path(subdivisions_path)?;
    let cnt_path = resolve_existing_path(countries_path)?;

    let sub_content = fs::read_to_string(sub_path).ok()?;
    let cnt_content = fs::read_to_string(cnt_path).ok()?;

    let sub_root: GeoJsonRoot = serde_json::from_str(&sub_content).ok()?;
    let cnt_root: GeoJsonRoot = serde_json::from_str(&cnt_content).ok()?;

    let mapping = load_mapping(Path::new("data/map/province_mapping.ron")).unwrap_or_default();

    let mut provinces = Vec::new();
    let mut fallback_id = 500u16;
    let mut suppressed_border_edges = HashSet::new();

    if !split_countries.is_empty() {
        for feat in sub_root.features {
            let adm0 = feat
                .properties
                .get("adm0_a3")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if split_countries.contains(&adm0) {
                let code = feat
                    .properties
                    .get("iso_3166_2")
                    .or_else(|| feat.properties.get("code_hasc"))
                    .and_then(|v| v.as_str())
                    .or_else(|| feat.properties.get("name").and_then(|v| v.as_str()))
                    .unwrap_or("");

                let prov_id = match mapping.subdivisions.get(code) {
                    Some(&id) => id,
                    None => {
                        let id = fallback_id;
                        fallback_id = fallback_id.saturating_add(1);
                        id
                    }
                };

                let name = feat
                    .properties
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Subdivision")
                    .to_string();

                let rings = extract_rings(&feat.geometry);
                for ring in rings {
                    let bbox = compute_bbox(&ring);
                    provinces.push(ProvinceVisualDef {
                        id: prov_id,
                        country_tag: adm0.to_string(),
                        name: name.clone(),
                        points: ring,
                        bbox,
                    });
                }
            }
        }
    }

    for feat in cnt_root.features {
        let iso = feat
            .properties
            .get("ISO_A3")
            .and_then(|v| v.as_str())
            .filter(|&s| s != "-99" && !s.is_empty())
            .or_else(|| {
                feat.properties
                    .get("ADM0_A3")
                    .and_then(|v| v.as_str())
                    .filter(|&s| s != "-99" && !s.is_empty())
            })
            .or_else(|| feat.properties.get("SOV_A3").and_then(|v| v.as_str()))
            .unwrap_or("");

        if split_countries.contains(&iso) {
            let rings = extract_rings(&feat.geometry);
            for ring in rings {
                let n = ring.len();
                for i in 0..n {
                    let next_i = (i + 1) % n;
                    let p0 = ring[i];
                    let p1 = ring[next_i];
                    let k0 = ((p0[0] * 2.0).round() as i32, (p0[1] * 2.0).round() as i32);
                    let k1 = ((p1[0] * 2.0).round() as i32, (p1[1] * 2.0).round() as i32);
                    if k0 != k1 {
                        let key = if k0 < k1 { (k0, k1) } else { (k1, k0) };
                        suppressed_border_edges.insert(key);
                    }
                }
            }
            continue;
        }

        if iso == "ATA" || iso.is_empty() {
            continue;
        }

        let prov_id = match mapping.countries.get(iso) {
            Some(&id) => id,
            None => {
                let id = fallback_id;
                fallback_id = fallback_id.saturating_add(1);
                id
            }
        };

        let name = feat
            .properties
            .get("NAME")
            .or_else(|| feat.properties.get("ADMIN"))
            .and_then(|v| v.as_str())
            .unwrap_or("Country")
            .to_string();

        let rings = extract_rings(&feat.geometry);
        for ring in rings {
            let bbox = compute_bbox(&ring);
            provinces.push(ProvinceVisualDef {
                id: prov_id,
                country_tag: iso.to_string(),
                name: name.clone(),
                points: ring,
                bbox,
            });
        }
    }

    if provinces.is_empty() {
        None
    } else {
        Some(MapDataDef {
            provinces,
            suppressed_border_edges,
        })
    }
}

fn load_mapping(path: &Path) -> Option<ProvinceMapping> {
    let resolved = resolve_existing_path(path)?;
    let content = fs::read_to_string(resolved).ok()?;
    let clean = content.trim_start_matches('\u{feff}');
    ron::from_str::<ProvinceMapping>(clean).ok()
}

fn compute_bbox(points: &[[f32; 2]]) -> [f32; 4] {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for p in points {
        min_x = min_x.min(p[0]);
        min_y = min_y.min(p[1]);
        max_x = max_x.max(p[0]);
        max_y = max_y.max(p[1]);
    }
    [min_x, min_y, max_x, max_y]
}

fn extract_rings(geom: &GeoJsonGeometry) -> Vec<Vec<[f32; 2]>> {
    let mut result = Vec::new();

    if geom.geom_type == "Polygon" {
        if let Some(poly) = geom.coordinates.as_array() {
            if let Some(outer_ring) = poly.first().and_then(|r| r.as_array()) {
                if let Some(pts) = parse_ring(outer_ring) {
                    result.push(pts);
                }
            }
        }
    } else if geom.geom_type == "MultiPolygon" {
        if let Some(multipoly) = geom.coordinates.as_array() {
            for poly in multipoly {
                if let Some(outer_ring) = poly
                    .as_array()
                    .and_then(|p| p.first())
                    .and_then(|r| r.as_array())
                {
                    if let Some(pts) = parse_ring(outer_ring) {
                        result.push(pts);
                    }
                }
            }
        }
    }

    result
}

fn calculate_signed_area(pts: &[[f32; 2]]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0f64;
    for i in 0..n {
        let j = (i + 1) % n;
        area += (pts[i][0] as f64) * (pts[j][1] as f64) - (pts[j][0] as f64) * (pts[i][1] as f64);
    }
    area * 0.5
}

fn parse_ring(coords: &[serde_json::Value]) -> Option<Vec<[f32; 2]>> {
    if coords.len() < 4 {
        return None;
    }

    let mut points: Vec<[f32; 2]> = Vec::with_capacity(coords.len());
    for item in coords {
        if let Some(arr) = item.as_array() {
            if arr.len() >= 2 {
                let lon = arr[0].as_f64()?;
                let lat = arr[1].as_f64()?;
                let pt = project_mercator(lon, lat);
                if let Some(last) = points.last() {
                    let dx = pt[0] - last[0];
                    let dy = pt[1] - last[1];
                    if dx * dx + dy * dy < 1e-6 {
                        continue;
                    }
                }
                points.push(pt);
            }
        }
    }

    if points.len() >= 2 {
        let first = points[0];
        let last = points[points.len() - 1];
        if (first[0] - last[0]).abs() < 1e-4 && (first[1] - last[1]).abs() < 1e-4 {
            points.pop();
        }
    }

    if points.len() >= 3 {
        if calculate_signed_area(&points) < 0.0 {
            points.reverse();
        }
        Some(points)
    } else {
        None
    }
}

pub fn project_mercator(lon: f64, lat: f64) -> [f32; 2] {
    let clamped_lat = lat.clamp(-80.0, 84.0);
    let lat_rad = clamped_lat.to_radians();
    let x = (lon * 15.0) as f32;
    let y = (-((std::f64::consts::FRAC_PI_4 + lat_rad * 0.5).tan().ln()) * 859.4367) as f32;
    [x, y]
}

fn resolve_existing_path(path: &Path) -> Option<PathBuf> {
    let candidates = [
        path.to_path_buf(),
        PathBuf::from("../../").join(path),
        PathBuf::from("../").join(path),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}
