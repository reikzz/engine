use crate::map::geometry::MapDataDef;
use glam::Vec2;

pub fn pick_province_at(world_pos: Vec2, map_data: &MapDataDef) -> Option<u16> {
    let mut wrapped_x = world_pos.x;
    while wrapped_x > 2700.0 {
        wrapped_x -= 5400.0;
    }
    while wrapped_x < -2700.0 {
        wrapped_x += 5400.0;
    }
    let pos = Vec2::new(wrapped_x, world_pos.y);

    for prov in &map_data.provinces {
        if pos.x < prov.bbox[0]
            || pos.x > prov.bbox[2]
            || pos.y < prov.bbox[1]
            || pos.y > prov.bbox[3]
        {
            continue;
        }
        if is_point_inside_polygon(pos, &prov.points) {
            return Some(prov.id);
        }
    }
    None
}

fn is_point_inside_polygon(point: Vec2, vertices: &[[f32; 2]]) -> bool {
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
