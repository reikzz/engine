use crate::bridge::commands::ClientCommand;
use crate::bridge::snapshot::WorldSnapshot;
use crate::debug::DebugOverlay;
use crate::map::camera::Camera2D;
use crate::map::country_names::{CountryLabelAnchor, CountryNamesDef};
use crate::map::geometry::{self, MapDataDef, MapMesh};
use crate::map::modes::MapMode;
use crate::map::picking::pick_province_at;
use crate::render::{MapPipeline, OverlayPipeline, WgpuContext};
use crate::ui::UiManager;
use crossbeam_channel::{Receiver, Sender};
use glam::Vec2;
use std::path::Path;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

pub struct App {
    window: Option<Arc<Window>>,
    wgpu_ctx: Option<WgpuContext>,
    map_pipeline: Option<MapPipeline>,
    overlay_pipeline: Option<OverlayPipeline>,
    ui_manager: Option<UiManager>,
    debug_overlay: Option<DebugOverlay>,
    camera: Camera2D,
    map_data: MapDataDef,
    mesh: MapMesh,
    active_map_mode: MapMode,
    selected_province: Option<u16>,
    last_selected_province: Option<u16>,
    selected_country: Option<String>,
    active_country: String,
    is_panning: bool,
    pan_drag_distance: f32,
    last_cursor_pos: Vec2,
    hovered_province: Option<u16>,
    command_sender: Sender<ClientCommand>,
    snapshot_receiver: Receiver<WorldSnapshot>,
    mode_sender: Sender<MapMode>,
    mode_receiver: Receiver<MapMode>,
    latest_snapshot: WorldSnapshot,
    country_names: CountryNamesDef,
    country_anchors: Vec<CountryLabelAnchor>,
    ui_dirty: bool,
}

impl App {
    pub fn new(
        command_sender: Sender<ClientCommand>,
        snapshot_receiver: Receiver<WorldSnapshot>,
        split_countries: &[&str],
    ) -> Self {
        let map_path = Path::new("data/provinces.ron");
        let country_names = CountryNamesDef::load(Path::new("data/map/country_names.ron"));
        let map_data = MapDataDef::load_or_generate(map_path, split_countries);
        let mesh = map_data.build_mesh();
        let (mode_sender, mode_receiver) = crossbeam_channel::unbounded();

        let mut camera = Camera2D::new(1280.0, 720.0);
        camera.position = Vec2::new(0.0, -100.0);
        camera.zoom = 0.22;

        Self {
            window: None,
            wgpu_ctx: None,
            map_pipeline: None,
            overlay_pipeline: None,
            ui_manager: None,
            debug_overlay: None,
            camera,
            map_data,
            mesh,
            active_map_mode: MapMode::Political,
            selected_province: None,
            last_selected_province: None,
            selected_country: None,
            active_country: "USA".to_string(),
            is_panning: false,
            pan_drag_distance: 0.0,
            last_cursor_pos: Vec2::ZERO,
            hovered_province: None,
            command_sender,
            snapshot_receiver,
            mode_sender,
            mode_receiver,
            latest_snapshot: WorldSnapshot::default(),
            country_names,
            country_anchors: Vec::new(),
            ui_dirty: true,
        }
    }

    fn resolve_province_country(&self, prov_id: u16) -> Option<String> {
        if let Some(prov) = self.latest_snapshot.provinces.get(&prov_id) {
            if let Some(reg) = self.latest_snapshot.regions.get(&prov.region_id) {
                if let Some(ref owner) = reg.owner {
                    return Some(owner.clone());
                }
            }
            if let Some(ref controller) = prov.controller {
                return Some(controller.clone());
            }
        }

        self.map_data
            .provinces
            .iter()
            .find(|p| p.id == prov_id)
            .map(|p| p.country_tag.clone())
    }

    fn is_province_in_selected_country(&self, prov_id: u16) -> bool {
        let selected = match self.selected_country {
            Some(ref c) => c,
            None => return false,
        };

        if let Some(country) = self.resolve_province_country(prov_id) {
            &country == selected
        } else {
            false
        }
    }

    fn compute_country_anchors(&self) -> Vec<CountryLabelAnchor> {
        let mut country_bounds: std::collections::BTreeMap<
            String,
            (glam::Vec2, glam::Vec2, glam::Vec2, u32),
        > = std::collections::BTreeMap::new();

        for (&prov_id, &center) in &self.mesh.province_centers {
            if let Some(country_key) = self.resolve_province_country(prov_id) {
                if !self.latest_snapshot.countries.contains_key(&country_key) {
                    continue;
                }
                let entry = country_bounds.entry(country_key).or_insert((
                    center,
                    center,
                    glam::Vec2::ZERO,
                    0,
                ));
                entry.0 = glam::Vec2::new(entry.0.x.min(center.x), entry.0.y.min(center.y));
                entry.1 = glam::Vec2::new(entry.1.x.max(center.x), entry.1.y.max(center.y));
                entry.2 += center;
                entry.3 += 1;
            }
        }

        let mut anchors = Vec::with_capacity(country_bounds.len());
        for (country_key, (bbox_min, bbox_max, sum, count)) in country_bounds {
            if count == 0 {
                continue;
            }
            let world_width = (bbox_max.x - bbox_min.x).max(20.0);
            let world_height = (bbox_max.y - bbox_min.y).max(20.0);

            let center = if let Some((lon, lat)) = self.country_names.resolve_anchor(&country_key) {
                let pt = crate::map::geo_loader::project_mercator(lon as f64, lat as f64);
                glam::Vec2::new(pt[0], pt[1])
            } else {
                sum / count as f32
            };

            let display_name = self
                .country_names
                .resolve_display_name(&country_key)
                .to_uppercase();
            anchors.push(CountryLabelAnchor {
                name: display_name,
                center,
                world_width,
                world_height,
            });
        }

        anchors
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_title("Engine Simulation Client")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

        let window = match event_loop.create_window(window_attributes) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("Failed to create window: {e}");
                return;
            }
        };

        let wgpu_ctx = match WgpuContext::new(window.clone()) {
            Ok(ctx) => ctx,
            Err(e) => {
                eprintln!("Failed to create wgpu context: {e}");
                return;
            }
        };

        let pipeline = MapPipeline::new(
            &wgpu_ctx.device,
            wgpu_ctx.config.format,
            &self.mesh,
            self.map_data.provinces.len().max(2048),
        );

        let overlay_pipeline = OverlayPipeline::new(&wgpu_ctx.device, wgpu_ctx.config.format);

        let ui_manager = match UiManager::new(
            self.command_sender.clone(),
            self.mode_sender.clone(),
            wgpu_ctx.size.0,
            wgpu_ctx.size.1,
        ) {
            Ok(ui) => Some(ui),
            Err(e) => {
                eprintln!("Failed to initialize Slint UI: {e}");
                None
            }
        };

        let debug_overlay = DebugOverlay::new(&wgpu_ctx.device, wgpu_ctx.config.format, &window);

        self.camera
            .resize(wgpu_ctx.size.0 as f32, wgpu_ctx.size.1 as f32);

        self.window = Some(window);
        self.wgpu_ctx = Some(wgpu_ctx);
        self.map_pipeline = Some(pipeline);
        self.overlay_pipeline = Some(overlay_pipeline);
        self.ui_manager = ui_manager;
        self.debug_overlay = Some(debug_overlay);

        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(16),
        ));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let window = match self.window.as_ref() {
            Some(w) => w,
            None => return,
        };

        if let Some(ref mut debug) = self.debug_overlay {
            if debug.handle_event(window, &event) {
                window.request_redraw();
                return;
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(ref mut ctx) = self.wgpu_ctx {
                    ctx.resize(new_size.width, new_size.height);
                    self.camera
                        .resize(new_size.width as f32, new_size.height as f32);
                    if let Some(ref mut overlay) = self.overlay_pipeline {
                        overlay.resize(&ctx.device, new_size.width, new_size.height);
                    }
                    if let Some(ref mut ui) = self.ui_manager {
                        ui.resize(new_size.width, new_size.height);
                    }
                }
                window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::F12) | PhysicalKey::Code(KeyCode::Backquote) => {
                            if let Some(ref mut debug) = self.debug_overlay {
                                debug.toggle();
                                window.request_redraw();
                            }
                        }
                        PhysicalKey::Code(KeyCode::Space) => {
                            let _ = self.command_sender.send(ClientCommand::ChangeSpeed(
                                crate::bridge::commands::SimulationSpeed::Paused,
                            ));
                        }
                        PhysicalKey::Code(KeyCode::Digit1) => {
                            let _ = self.command_sender.send(ClientCommand::ChangeSpeed(
                                crate::bridge::commands::SimulationSpeed::Speed1x,
                            ));
                        }
                        PhysicalKey::Code(KeyCode::Digit2) => {
                            let _ = self.command_sender.send(ClientCommand::ChangeSpeed(
                                crate::bridge::commands::SimulationSpeed::Speed2x,
                            ));
                        }
                        PhysicalKey::Code(KeyCode::Digit3) => {
                            let _ = self.command_sender.send(ClientCommand::ChangeSpeed(
                                crate::bridge::commands::SimulationSpeed::Speed5x,
                            ));
                        }
                        PhysicalKey::Code(KeyCode::Digit4) => {
                            let _ = self.command_sender.send(ClientCommand::ChangeSpeed(
                                crate::bridge::commands::SimulationSpeed::MaxSpeed,
                            ));
                        }
                        PhysicalKey::Code(KeyCode::KeyQ) => {
                            self.camera.rotate(0.08);
                            window.request_redraw();
                        }
                        PhysicalKey::Code(KeyCode::KeyE) => {
                            self.camera.rotate(-0.08);
                            window.request_redraw();
                        }
                        PhysicalKey::Code(KeyCode::KeyR) => {
                            self.camera.reset_rotation();
                            window.request_redraw();
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let new_pos = Vec2::new(position.x as f32, position.y as f32);
                let delta = self.last_cursor_pos - new_pos;

                if self.is_panning {
                    self.camera.pan(delta);
                    self.pan_drag_distance += delta.length();
                }

                if let Some(ref ui) = self.ui_manager {
                    ui.forward_pointer_move(new_pos.x, new_pos.y);
                }

                let over_ui = self
                    .ui_manager
                    .as_ref()
                    .map(|ui| ui.is_point_over_ui(new_pos.x, new_pos.y))
                    .unwrap_or(false);

                if !over_ui && !self.is_panning {
                    let world_pos = self.camera.screen_to_world(new_pos);
                    let new_hover = pick_province_at(world_pos, &self.map_data);
                    if new_hover != self.hovered_province {
                        self.hovered_province = new_hover;
                        window.request_redraw();
                    }
                } else if self.hovered_province.is_some() {
                    self.hovered_province = None;
                    window.request_redraw();
                }

                self.last_cursor_pos = new_pos;
                window.request_redraw();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let is_pressed = state == ElementState::Pressed;

                if button == MouseButton::Middle {
                    self.is_panning = is_pressed;
                } else if button == MouseButton::Right {
                    if is_pressed {
                        self.is_panning = true;
                        self.pan_drag_distance = 0.0;
                    } else {
                        self.is_panning = false;
                        let over_ui = self
                            .ui_manager
                            .as_ref()
                            .map(|ui| {
                                ui.is_point_over_ui(self.last_cursor_pos.x, self.last_cursor_pos.y)
                            })
                            .unwrap_or(false);

                        if !over_ui && self.pan_drag_distance < 6.0 {
                            let world_pos = self.camera.screen_to_world(self.last_cursor_pos);
                            if let Some(prov_id) = pick_province_at(world_pos, &self.map_data) {
                                if let Some(owner) = self.resolve_province_country(prov_id) {
                                    self.selected_country = Some(owner);
                                    self.selected_province = None;
                                    let _ = self
                                        .command_sender
                                        .send(ClientCommand::InspectRegion(None));
                                    self.ui_dirty = true;
                                } else {
                                    self.selected_country = None;
                                    self.ui_dirty = true;
                                }
                            } else {
                                self.selected_country = None;
                                self.ui_dirty = true;
                            }
                        }
                    }
                } else if button == MouseButton::Left {
                    let over_ui = self
                        .ui_manager
                        .as_ref()
                        .map(|ui| {
                            ui.is_point_over_ui(self.last_cursor_pos.x, self.last_cursor_pos.y)
                        })
                        .unwrap_or(false);

                    if over_ui {
                        if let Some(ref ui) = self.ui_manager {
                            if is_pressed {
                                ui.forward_pointer_press(
                                    self.last_cursor_pos.x,
                                    self.last_cursor_pos.y,
                                );
                            } else {
                                ui.forward_pointer_release(
                                    self.last_cursor_pos.x,
                                    self.last_cursor_pos.y,
                                );
                            }
                            self.ui_dirty = true;
                        }
                    } else if is_pressed {
                        let world_pos = self.camera.screen_to_world(self.last_cursor_pos);
                        self.selected_province = pick_province_at(world_pos, &self.map_data);
                        self.selected_country = None;
                        if let Some(prov_id) = self.selected_province {
                            if let Some(owner) = self.resolve_province_country(prov_id) {
                                self.active_country = owner;
                            }
                            let region_id = self
                                .latest_snapshot
                                .provinces
                                .get(&prov_id)
                                .map(|p| p.region_id);
                            let _ = self
                                .command_sender
                                .send(ClientCommand::InspectRegion(region_id));
                        } else {
                            let _ = self.command_sender.send(ClientCommand::InspectRegion(None));
                        }
                        self.ui_dirty = true;
                    }
                }
                window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => (pos.y as f32) * 0.05,
                };

                let factor = if scroll_y > 0.0 { 1.15 } else { 0.85 };
                self.camera.zoom_at(factor, self.last_cursor_pos);
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let mut got_update = false;
        while let Ok(snapshot) = self.snapshot_receiver.try_recv() {
            self.latest_snapshot = snapshot;
            self.ui_dirty = true;
            got_update = true;
        }

        while let Ok(mode) = self.mode_receiver.try_recv() {
            self.active_map_mode = mode;
            self.ui_dirty = true;
            got_update = true;
        }

        if got_update {
            if self.country_anchors.is_empty() && !self.latest_snapshot.countries.is_empty() {
                self.country_anchors = self.compute_country_anchors();
            }
            if let Some(ref window) = self.window {
                window.request_redraw();
            }
        }

        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(16),
        ));
    }
}

impl App {
    fn render(&mut self) {
        let ctx = match self.wgpu_ctx.as_ref() {
            Some(c) => c,
            None => return,
        };
        let window = match self.window.as_ref() {
            Some(w) => w,
            None => return,
        };

        if self.country_anchors.is_empty() && !self.latest_snapshot.countries.is_empty() {
            self.country_anchors = self.compute_country_anchors();
        }

        if let Some(ref mut ui) = self.ui_manager {
            if ui.take_country_menu_closed() {
                self.selected_country = None;
                self.ui_dirty = true;
            }
            if ui.take_region_panel_closed() {
                self.selected_province = None;
                let _ = self.command_sender.send(ClientCommand::InspectRegion(None));
                self.ui_dirty = true;
            }

            if self.ui_dirty {
                let country_display_name = self
                    .country_names
                    .resolve_display_name(&self.active_country);

                let inspected = self.selected_country.as_deref().map(|tag| {
                    let name = self.country_names.resolve_display_name(tag);
                    (tag, name)
                });

                ui.update_state(
                    &self.latest_snapshot,
                    self.selected_province,
                    &self.active_country,
                    country_display_name,
                    inspected,
                );
                self.ui_dirty = false;
            }

            ui.update_country_labels(&self.camera, &self.country_anchors);
        }

        let max_prov_id = self
            .map_data
            .provinces
            .iter()
            .map(|p| p.id)
            .max()
            .unwrap_or(0) as usize;
        let mut colors = vec![[0.22f32, 0.24f32, 0.28f32, 0.75f32]; (max_prov_id + 1).max(2048)];
        for prov in &self.map_data.provinces {
            let is_selected_prov = Some(prov.id) == self.selected_province;
            let is_selected_country = self.is_province_in_selected_country(prov.id);
            let is_hovered = Some(prov.id) == self.hovered_province;
            let mut color = self.active_map_mode.calculate_province_color(
                prov.id,
                &prov.country_tag,
                &self.latest_snapshot,
                &self.country_names,
                is_selected_prov,
                is_selected_country,
            );
            if is_hovered && !is_selected_prov && !is_selected_country {
                color[0] = (color[0] * 1.15 + 0.06).min(1.0);
                color[1] = (color[1] * 1.15 + 0.06).min(1.0);
                color[2] = (color[2] * 1.15 + 0.06).min(1.0);
            }
            if (prov.id as usize) < colors.len() {
                colors[prov.id as usize] = color;
            }
        }

        if let Some(ref mut map_pipe) = self.map_pipeline {
            map_pipe.update_camera(&ctx.queue, &self.camera);
            map_pipe.update_colors(&ctx.queue, &colors);

            if self.selected_province != self.last_selected_province {
                self.last_selected_province = self.selected_province;
                if let Some(prov_id) = self.selected_province {
                    let (v, i) =
                        geometry::build_selection_outline_mesh(&self.map_data.provinces, prov_id);
                    map_pipe.update_selection(&ctx.queue, &v, &i);
                } else {
                    map_pipe.clear_selection();
                }
            }
        }

        if let (Some(ui), Some(overlay)) = (&mut self.ui_manager, &self.overlay_pipeline) {
            ui.render(overlay, &ctx.queue);
        }

        let output = match ctx.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost) => {
                ctx.surface.configure(&ctx.device, &ctx.config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                eprintln!("Out of memory when acquiring surface texture");
                return;
            }
            Err(e) => {
                eprintln!("Surface error: {e:?}");
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("client_render_encoder"),
            });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("map_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.07,
                            g: 0.11,
                            b: 0.18,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(ref map_pipe) = self.map_pipeline {
                map_pipe.draw(&mut rpass);
            }
        }

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overlay_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(ref overlay) = self.overlay_pipeline {
                overlay.draw(&mut rpass);
            }
        }

        if let Some(ref mut debug) = self.debug_overlay {
            debug.render(
                &ctx.device,
                &ctx.queue,
                &mut encoder,
                &view,
                window,
                ctx.size.0,
                ctx.size.1,
                &self.latest_snapshot,
            );
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}
