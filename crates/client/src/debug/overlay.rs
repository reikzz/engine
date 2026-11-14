use crate::bridge::snapshot::WorldSnapshot;
use egui_wgpu::{Renderer, ScreenDescriptor};
use egui_winit::State;
use winit::event::WindowEvent;
use winit::window::Window;

pub struct DebugOverlay {
    pub is_visible: bool,
    egui_ctx: egui::Context,
    egui_state: State,
    renderer: Renderer,
}

impl DebugOverlay {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, window: &Window) -> Self {
        let egui_ctx = egui::Context::default();
        let viewport_id = egui_ctx.viewport_id();
        let egui_state = State::new(egui_ctx.clone(), viewport_id, window, None, None, None);
        let renderer = Renderer::new(device, format, None, 1, false);

        Self {
            is_visible: false,
            egui_ctx,
            egui_state,
            renderer,
        }
    }

    pub fn toggle(&mut self) {
        self.is_visible = !self.is_visible;
    }

    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        if !self.is_visible {
            return false;
        }
        let response = self.egui_state.on_window_event(window, event);
        response.consumed
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        window: &Window,
        width: u32,
        height: u32,
        snapshot: &WorldSnapshot,
    ) {
        if !self.is_visible {
            return;
        }

        let raw_input = self.egui_state.take_egui_input(window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            egui::Window::new("Engine Profiler & Debug")
                .default_width(420.0)
                .default_height(500.0)
                .show(ctx, |ui| {
                    ui.heading("Системный профилировщик тика");
                    ui.label(format!(
                        "Скорость симуляции: {:.1} TPS",
                        snapshot.ticks_per_second
                    ));
                    ui.label(format!("Всего тиков (часов): {}", snapshot.total_ticks));
                    ui.label(format!("Дата: {}", snapshot.date_display));
                    ui.separator();

                    ui.heading("Метрики систем (SystemPipeline):");
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            egui::Grid::new("systems_grid")
                                .striped(true)
                                .min_col_width(120.0)
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new("Система").strong());
                                    ui.label(egui::RichText::new("Посл. (мкс)").strong());
                                    ui.label(egui::RichText::new("Средн. (мкс)").strong());
                                    ui.end_row();

                                    for item in &snapshot.system_metrics {
                                        ui.label(&item.name);
                                        ui.label(format!("{}", item.last_duration_micros));
                                        ui.label(format!("{:.1}", item.average_duration_micros));
                                        ui.end_row();
                                    }
                                });
                        });

                    ui.separator();
                    ui.heading("Сущности мира:");
                    ui.label(format!("Стран в симуляции: {}", snapshot.countries.len()));
                    ui.label(format!("Регионов: {}", snapshot.regions.len()));
                    ui.label(format!("Провинций: {}", snapshot.provinces.len()));
                });
        });

        self.egui_state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        for (id, delta) in &full_output.textures_delta.set {
            self.renderer.update_texture(device, queue, *id, delta);
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: full_output.pixels_per_point,
        };

        self.renderer
            .update_buffers(device, queue, encoder, &tris, &screen_descriptor);

        {
            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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

            self.renderer
                .render(&mut rpass.forget_lifetime(), &tris, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }
}
