use glam::{Mat4, Vec2, Vec4};

#[derive(Debug, Clone)]
pub struct Camera2D {
    pub position: Vec2,
    pub zoom: f32,
    pub rotation: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
            rotation: 0.0,
            viewport_width: 1280.0,
            viewport_height: 720.0,
        }
    }
}

impl Camera2D {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
            rotation: 0.0,
            viewport_width: width.max(1.0),
            viewport_height: height.max(1.0),
        }
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.viewport_width = width.max(1.0);
        self.viewport_height = height.max(1.0);
    }

    pub fn pan(&mut self, delta: Vec2) {
        let cos_r = self.rotation.cos();
        let sin_r = self.rotation.sin();
        let rotated_delta = Vec2::new(
            delta.x * cos_r - delta.y * sin_r,
            delta.x * sin_r + delta.y * cos_r,
        );
        let world_delta = rotated_delta / self.zoom;
        self.position += world_delta;

        if self.position.x > 2700.0 {
            self.position.x -= 5400.0;
        } else if self.position.x < -2700.0 {
            self.position.x += 5400.0;
        }
        self.position.y = self.position.y.clamp(-2300.0, 1900.0);
    }

    pub fn zoom_at(&mut self, factor: f32, screen_focus: Vec2) {
        let old_world_focus = self.screen_to_world(screen_focus);
        self.zoom = (self.zoom * factor).clamp(0.12, 4.0);
        let new_world_focus = self.screen_to_world(screen_focus);
        self.position += old_world_focus - new_world_focus;

        if self.position.x > 2700.0 {
            self.position.x -= 5400.0;
        } else if self.position.x < -2700.0 {
            self.position.x += 5400.0;
        }
        self.position.y = self.position.y.clamp(-2300.0, 1900.0);
    }

    pub fn rotate(&mut self, delta_rad: f32) {
        self.rotation += delta_rad;
        let tau = std::f32::consts::TAU;
        self.rotation = (self.rotation % tau + tau) % tau;
    }

    pub fn reset_rotation(&mut self) {
        self.rotation = 0.0;
    }

    pub fn build_matrix(&self) -> Mat4 {
        let half_w = (self.viewport_width * 0.5) / self.zoom;
        let half_h = (self.viewport_height * 0.5) / self.zoom;

        let left = -half_w;
        let right = half_w;
        let bottom = half_h;
        let top = -half_h;

        let ortho = Mat4::orthographic_rh(left, right, bottom, top, -1.0, 1.0);
        let rot = Mat4::from_rotation_z(self.rotation);
        let trans =
            Mat4::from_translation(glam::Vec3::new(-self.position.x, -self.position.y, 0.0));

        ortho * rot * trans
    }

    pub fn screen_to_world(&self, screen_coords: Vec2) -> Vec2 {
        let ndc_x = (screen_coords.x / self.viewport_width) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_coords.y / self.viewport_height) * 2.0;

        let inv_mat = self.build_matrix().inverse();
        let world_vec = inv_mat * Vec4::new(ndc_x, ndc_y, 0.0, 1.0);

        Vec2::new(world_vec.x, world_vec.y)
    }

    pub fn world_to_screen(&self, world_coords: Vec2) -> Vec2 {
        let mat = self.build_matrix();
        let clip = mat * Vec4::new(world_coords.x, world_coords.y, 0.0, 1.0);

        let screen_x = ((clip.x + 1.0) * 0.5) * self.viewport_width;
        let screen_y = ((1.0 - clip.y) * 0.5) * self.viewport_height;

        Vec2::new(screen_x, screen_y)
    }
}
