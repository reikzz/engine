use crate::map::camera::Camera2D;
use crate::map::geometry::{MapMesh, MapVertex};
use bytemuck::{Pod, Zeroable, cast_slice};
use glam::Mat4;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniformRaw {
    view_proj: [f32; 16],
    internal_border_alpha: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

pub struct MapPipeline {
    ocean_pipeline: wgpu::RenderPipeline,
    fill_pipeline: wgpu::RenderPipeline,
    border_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    fill_index_buffer: wgpu::Buffer,
    fill_index_count: u32,
    border_index_buffer: wgpu::Buffer,
    border_index_count: u32,
    selection_vertex_buffer: wgpu::Buffer,
    selection_index_buffer: wgpu::Buffer,
    selection_index_count: u32,
    camera_buffer: wgpu::Buffer,
    colors_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl MapPipeline {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        mesh: &MapMesh,
        max_provinces: usize,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("map_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/map.wgsl").into()),
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("map_vertex_buffer"),
            contents: cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let fill_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("map_fill_index_buffer"),
            contents: cast_slice(&mesh.fill_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let border_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("map_border_index_buffer"),
            contents: cast_slice(&mesh.border_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let max_selection_vertices = 16384;
        let max_selection_indices = 49152;
        let selection_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map_selection_vertex_buffer"),
            size: (max_selection_vertices * std::mem::size_of::<MapVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let selection_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("map_selection_index_buffer"),
            size: (max_selection_indices * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let initial_camera = CameraUniformRaw {
            view_proj: Mat4::IDENTITY.to_cols_array(),
            internal_border_alpha: 1.0,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("map_camera_buffer"),
            contents: bytemuck::bytes_of(&initial_camera),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let default_colors = vec![[0.22f32, 0.25f32, 0.30f32, 0.85f32]; max_provinces.max(2048)];
        let colors_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("map_colors_buffer"),
            contents: cast_slice(&default_colors),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("map_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("map_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: colors_buffer.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("map_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MapVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Uint32,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Uint32,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        };

        let ocean_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("map_ocean_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_ocean"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_ocean"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let fill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("map_fill_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout.clone()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let border_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("map_border_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            ocean_pipeline,
            fill_pipeline,
            border_pipeline,
            vertex_buffer,
            fill_index_buffer,
            fill_index_count: mesh.fill_indices.len() as u32,
            border_index_buffer,
            border_index_count: mesh.border_indices.len() as u32,
            selection_vertex_buffer,
            selection_index_buffer,
            selection_index_count: 0,
            camera_buffer,
            colors_buffer,
            bind_group,
        }
    }

    pub fn update_selection(
        &mut self,
        queue: &wgpu::Queue,
        vertices: &[MapVertex],
        indices: &[u32],
    ) {
        if vertices.is_empty() || indices.is_empty() {
            self.selection_index_count = 0;
            return;
        }
        let max_v =
            (self.selection_vertex_buffer.size() as usize) / std::mem::size_of::<MapVertex>();
        let max_i = (self.selection_index_buffer.size() as usize) / std::mem::size_of::<u32>();
        let v_slice = if vertices.len() > max_v {
            &vertices[..max_v]
        } else {
            vertices
        };
        let i_slice = if indices.len() > max_i {
            &indices[..max_i]
        } else {
            indices
        };

        queue.write_buffer(&self.selection_vertex_buffer, 0, cast_slice(v_slice));
        queue.write_buffer(&self.selection_index_buffer, 0, cast_slice(i_slice));
        self.selection_index_count = i_slice.len() as u32;
    }

    pub fn clear_selection(&mut self) {
        self.selection_index_count = 0;
    }

    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &Camera2D) {
        let mat = camera.build_matrix();
        let border_alpha = ((camera.zoom - 0.65) / 0.25).clamp(0.0, 1.0);
        let raw = CameraUniformRaw {
            view_proj: mat.to_cols_array(),
            internal_border_alpha: border_alpha,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&raw));
    }

    pub fn update_colors(&self, queue: &wgpu::Queue, colors: &[[f32; 4]]) {
        let max_elements = (self.colors_buffer.size() as usize) / std::mem::size_of::<[f32; 4]>();
        let slice = if colors.len() > max_elements {
            &colors[..max_elements]
        } else {
            colors
        };
        queue.write_buffer(&self.colors_buffer, 0, cast_slice(slice));
    }

    pub fn draw<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        rpass.set_bind_group(0, &self.bind_group, &[]);

        rpass.set_pipeline(&self.ocean_pipeline);
        rpass.draw(0..6, 0..1);

        if self.fill_index_count > 0 {
            rpass.set_pipeline(&self.fill_pipeline);
            rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rpass.set_index_buffer(self.fill_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            rpass.draw_indexed(0..self.fill_index_count, 0, 0..3);
        }

        if self.border_index_count > 0 {
            rpass.set_pipeline(&self.border_pipeline);
            rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rpass.set_index_buffer(
                self.border_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            rpass.draw_indexed(0..self.border_index_count, 0, 0..3);
        }

        if self.selection_index_count > 0 {
            rpass.set_pipeline(&self.border_pipeline);
            rpass.set_vertex_buffer(0, self.selection_vertex_buffer.slice(..));
            rpass.set_index_buffer(
                self.selection_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            rpass.draw_indexed(0..self.selection_index_count, 0, 0..3);
        }
    }
}
