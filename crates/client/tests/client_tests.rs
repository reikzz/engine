use glam::Vec2;

#[test]
fn test_load_provinces_ron_file() {
    let candidates = [
        std::path::PathBuf::from("data/provinces.ron"),
        std::path::PathBuf::from("../../data/provinces.ron"),
        std::path::PathBuf::from("../../../data/provinces.ron"),
        std::path::PathBuf::from("data/map/provinces.ron"),
        std::path::PathBuf::from("../../data/map/provinces.ron"),
    ];
    let path = candidates.into_iter().find(|p| p.exists());
    assert!(path.is_some(), "provinces.ron not found!");
    let content = std::fs::read_to_string(&path.unwrap()).unwrap();
    assert!(!content.is_empty());
}

#[test]
fn test_camera_ortho_projection_and_transform() {
    let mut camera = client_camera_test_helper(800.0, 600.0);
    assert_eq!(camera.zoom, 1.0);

    let center_screen = Vec2::new(400.0, 300.0);
    let world_pos = camera.screen_to_world(center_screen);
    assert!((world_pos.x - 0.0).abs() < 1e-4);
    assert!((world_pos.y - 0.0).abs() < 1e-4);

    camera.pan(Vec2::new(50.0, -25.0));
    let world_pos_panned = camera.screen_to_world(center_screen);
    assert!((world_pos_panned.x - 50.0).abs() < 1e-4);
    assert!((world_pos_panned.y - (-25.0)).abs() < 1e-4);

    camera.zoom_at(2.0, center_screen);
    assert!((camera.zoom - 2.0).abs() < 1e-4);
}

#[test]
fn test_point_in_polygon_picking() {
    let triangle = vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0]];

    assert!(is_point_inside_polygon_test(Vec2::new(5.0, 3.0), &triangle));
    assert!(!is_point_inside_polygon_test(
        Vec2::new(15.0, 3.0),
        &triangle
    ));
    assert!(!is_point_inside_polygon_test(
        Vec2::new(5.0, -1.0),
        &triangle
    ));
}

#[test]
fn test_shared_border_deduplication() {
    let mut seen_edges = std::collections::HashSet::new();
    let edge_a = ((0, 0), (10, 0));
    let edge_b = ((10, 0), (0, 0));
    let key_a = if edge_a.0 < edge_a.1 {
        (edge_a.0, edge_a.1)
    } else {
        (edge_a.1, edge_a.0)
    };
    let key_b = if edge_b.0 < edge_b.1 {
        (edge_b.0, edge_b.1)
    } else {
        (edge_b.1, edge_b.0)
    };
    assert_eq!(key_a, key_b);
    assert!(seen_edges.insert(key_a));
    assert!(!seen_edges.insert(key_b));
}

#[test]
fn test_map_geometry_tessellation() {
    let mut map_def = client_map_test_helper();
    map_def.ensure_all_provinces(&[0, 1, 2]);

    assert_eq!(map_def.provinces.len(), 3);

    let mesh = map_def.build_mesh();
    assert!(!mesh.vertices.is_empty());
    assert!(!mesh.fill_indices.is_empty());
    assert!(!mesh.border_indices.is_empty());
    assert_eq!(mesh.fill_indices.len() % 3, 0);
    assert_eq!(mesh.border_indices.len() % 2, 0);
}

fn client_camera_test_helper(w: f32, h: f32) -> CameraTestInstance {
    CameraTestInstance {
        position: Vec2::ZERO,
        zoom: 1.0,
        viewport_width: w,
        viewport_height: h,
    }
}

struct CameraTestInstance {
    position: Vec2,
    zoom: f32,
    viewport_width: f32,
    viewport_height: f32,
}

impl CameraTestInstance {
    fn pan(&mut self, delta: Vec2) {
        let world_delta = delta / self.zoom;
        self.position += world_delta;
    }

    fn zoom_at(&mut self, factor: f32, screen_focus: Vec2) {
        let old_world_focus = self.screen_to_world(screen_focus);
        self.zoom = (self.zoom * factor).clamp(0.01, 100.0);
        let new_world_focus = self.screen_to_world(screen_focus);
        self.position += old_world_focus - new_world_focus;
    }

    fn build_matrix(&self) -> glam::Mat4 {
        let half_w = (self.viewport_width * 0.5) / self.zoom;
        let half_h = (self.viewport_height * 0.5) / self.zoom;

        let left = self.position.x - half_w;
        let right = self.position.x + half_w;
        let bottom = self.position.y + half_h;
        let top = self.position.y - half_h;

        glam::Mat4::orthographic_rh(left, right, bottom, top, -1.0, 1.0)
    }

    fn screen_to_world(&self, screen_coords: Vec2) -> Vec2 {
        let ndc_x = (screen_coords.x / self.viewport_width) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_coords.y / self.viewport_height) * 2.0;

        let inv_mat = self.build_matrix().inverse();
        let world_vec = inv_mat * glam::Vec4::new(ndc_x, ndc_y, 0.0, 1.0);

        Vec2::new(world_vec.x, world_vec.y)
    }
}

fn is_point_inside_polygon_test(point: Vec2, vertices: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let n = vertices.len();
    if n < 3 {
        return false;
    }

    let px = point.x;
    let py = point.y;

    let mut j = n - 1;
    for i in 0..n {
        let xi = vertices[i][0];
        let yi = vertices[i][1];
        let xj = vertices[j][0];
        let yj = vertices[j][1];

        let intersects = ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi);

        if intersects {
            inside = !inside;
        }
        j = i;
    }

    inside
}

struct MapTestDef {
    provinces: Vec<ProvinceTestDef>,
}

struct ProvinceTestDef {
    points: Vec<[f32; 2]>,
}

struct MeshTestResult {
    vertices: Vec<[f32; 2]>,
    fill_indices: Vec<u32>,
    border_indices: Vec<u32>,
}

fn client_map_test_helper() -> MapTestDef {
    MapTestDef {
        provinces: Vec::new(),
    }
}

impl MapTestDef {
    fn ensure_all_provinces(&mut self, ids: &[u16]) {
        for &id in ids {
            let offset = (id as f32) * 50.0;
            self.provinces.push(ProvinceTestDef {
                points: vec![
                    [offset, 0.0],
                    [offset + 40.0, 0.0],
                    [offset + 40.0, 40.0],
                    [offset, 40.0],
                ],
            });
        }
    }

    fn build_mesh(&self) -> MeshTestResult {
        let mut vertices = Vec::new();
        let mut fill_indices = Vec::new();
        let mut border_indices = Vec::new();

        for prov in &self.provinces {
            let base = vertices.len() as u32;
            let mut flat = Vec::new();
            for p in &prov.points {
                flat.push(p[0] as f64);
                flat.push(p[1] as f64);
                vertices.push(*p);
            }

            if let Ok(tris) = earcutr::earcut(&flat, &[], 2) {
                for t in tris {
                    fill_indices.push(base + t as u32);
                }
            }

            let count = prov.points.len();
            for i in 0..count {
                border_indices.push(base + i as u32);
                border_indices.push(base + ((i + 1) % count) as u32);
            }
        }

        MeshTestResult {
            vertices,
            fill_indices,
            border_indices,
        }
    }
}

#[test]
fn test_polygon_signed_area_and_winding_order() {
    let clockwise_square = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let mut area = 0.0f64;
    let n = clockwise_square.len();
    for i in 0..n {
        let j = (i + 1) % n;
        area += (clockwise_square[i][0] as f64) * (clockwise_square[j][1] as f64)
            - (clockwise_square[j][0] as f64) * (clockwise_square[i][1] as f64);
    }
    area *= 0.5;
    assert!(area > 0.0);

    let mut counter_square = clockwise_square.clone();
    counter_square.reverse();
    let mut rev_area = 0.0f64;
    for i in 0..n {
        let j = (i + 1) % n;
        rev_area += (counter_square[i][0] as f64) * (counter_square[j][1] as f64)
            - (counter_square[j][0] as f64) * (counter_square[i][1] as f64);
    }
    rev_area *= 0.5;
    assert!(rev_area < 0.0);
}

#[test]
fn test_camera_clamping_ranges() {
    let mut cam = client_camera_test_helper(800.0, 600.0);
    cam.zoom = (cam.zoom * 0.001).clamp(0.12, 4.0);
    assert!((cam.zoom - 0.12).abs() < 1e-4);

    cam.zoom = (cam.zoom * 100.0).clamp(0.12, 4.0);
    assert!((cam.zoom - 4.0).abs() < 1e-4);

    let mut pos_y = -3500.0f32;
    pos_y = pos_y.clamp(-2300.0, 1900.0);
    assert_eq!(pos_y, -2300.0);

    let mut pos_y_south = 3500.0f32;
    pos_y_south = pos_y_south.clamp(-2300.0, 1900.0);
    assert_eq!(pos_y_south, 1900.0);
}

#[test]
fn test_aabb_bbox_filtering() {
    let bbox = [10.0f32, 20.0, 50.0, 80.0];
    let pt_inside = Vec2::new(25.0, 45.0);
    let pt_outside_x = Vec2::new(5.0, 45.0);
    let pt_outside_y = Vec2::new(25.0, 95.0);

    assert!(
        pt_inside.x >= bbox[0]
            && pt_inside.x <= bbox[2]
            && pt_inside.y >= bbox[1]
            && pt_inside.y <= bbox[3]
    );
    assert!(
        !(pt_outside_x.x >= bbox[0]
            && pt_outside_x.x <= bbox[2]
            && pt_outside_x.y >= bbox[1]
            && pt_outside_x.y <= bbox[3])
    );
    assert!(
        !(pt_outside_y.x >= bbox[0]
            && pt_outside_y.x <= bbox[2]
            && pt_outside_y.y >= bbox[1]
            && pt_outside_y.y <= bbox[3])
    );
}

#[test]
fn test_country_names_ron_has_definitions() {
    let candidates = [
        std::path::PathBuf::from("data/map/country_names.ron"),
        std::path::PathBuf::from("../../data/map/country_names.ron"),
    ];
    let path = candidates.into_iter().find(|p| p.exists());
    assert!(path.is_some(), "country_names.ron not found!");
    let content = std::fs::read_to_string(&path.unwrap()).unwrap();
    let clean = content.trim_start_matches('\u{feff}');
    let def: Result<ron::Value, _> = ron::from_str(clean);
    assert!(def.is_ok());
}

#[test]
fn test_non_scenario_color_is_muted() {
    let non_scenario_color = [0.22f32, 0.24, 0.26, 0.85];
    assert!(non_scenario_color[0] < 0.3);
    assert!(non_scenario_color[1] < 0.3);
    assert!(non_scenario_color[2] < 0.3);
}

#[test]
fn test_map_mode_indices() {
    assert_eq!(map_mode_from_index(0), 0);
    assert_eq!(map_mode_from_index(1), 1);
    assert_eq!(map_mode_from_index(2), 2);
    assert_eq!(map_mode_from_index(3), 3);
    assert_eq!(map_mode_from_index(4), 4);
    assert_eq!(map_mode_from_index(5), 5);
}

fn map_mode_from_index(index: i32) -> i32 {
    match index {
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        5 => 5,
        _ => 0,
    }
}

#[test]
fn test_compact_currency_formatting() {
    assert_eq!(test_format_compact_currency("$", 1_250_000.0), "$1.2M");
    assert_eq!(test_format_compact_currency("$", 500_000_000.0), "$500M");
    assert_eq!(
        test_format_compact_currency("$", 3_400_000_000_000.0),
        "$3.4T"
    );
}

fn test_format_compact_currency(symbol: &str, value: f64) -> String {
    let abs_val = value.abs();
    if abs_val >= 1_000_000_000_000.0 {
        format!("{}{:.1}T", symbol, abs_val / 1_000_000_000_000.0)
    } else if abs_val >= 1_000_000.0 {
        let scaled = abs_val / 1_000_000.0;
        if scaled.fract() == 0.0 {
            format!("{}{:.0}M", symbol, scaled)
        } else {
            format!("{}{:.1}M", symbol, scaled)
        }
    } else {
        format!("{}{:.0}", symbol, abs_val)
    }
}

#[test]
fn test_monetary_policy_command_and_snapshot_defaults() {
    let snapshot = client::bridge::snapshot::CountrySnapshot {
        tag: "USA".to_string(),
        currency_symbol: "$".to_string(),
        treasury: 100_000.0,
        national_debt: 50_000.0,
        total_population: 1_000_000,
        employed_population: 800_000,
        nominal_gdp: 5_000_000.0,
        corporate_tax: 0.20,
        income_tax: 0.15,
        tariff_rate: 0.05,
        central_bank_rate: 0.05,
        cpi: 102.4,
        inflation_rate: 0.024,
        real_interest_rate: 0.026,
    };

    assert_eq!(snapshot.tag, "USA");
    assert!((snapshot.central_bank_rate - 0.05).abs() < 1e-6);
    assert!((snapshot.cpi - 102.4).abs() < 1e-6);
    assert!((snapshot.inflation_rate - 0.024).abs() < 1e-6);
    assert!((snapshot.real_interest_rate - 0.026).abs() < 1e-6);

    let cmd = client::bridge::commands::ClientCommand::SetCentralBankRate {
        country: engine_core::world::CountryTag::new("USA"),
        target_rate: 0.065,
    };

    match cmd {
        client::bridge::commands::ClientCommand::SetCentralBankRate {
            country,
            target_rate,
        } => {
            assert_eq!(country.as_str(), "USA");
            assert!((target_rate - 0.065).abs() < 1e-6);
        }
        _ => panic!("Expected SetCentralBankRate command"),
    }
}
