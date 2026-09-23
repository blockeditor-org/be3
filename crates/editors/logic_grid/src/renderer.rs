use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use beui::{Draw, DrawAt, Drawing};
use bytemuck::{Pod, Zeroable};

use crate::frame::{
    RenderFrame, RenderVertex, WireValue, WireVertex, background_triangles, ray_vertices,
    stub_vertices, triangle_vertices, value_triangle_vertices, wire_vertices,
};

impl RenderVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

impl WireVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            0 => Float32x2,
            2 => Float32,
            3 => Float32,
            4 => Uint32,
            5 => Float32x4
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Placement {
    rect: [f32; 4],
    clip: [f32; 4],
    screen: [f32; 2],
    padding: [f32; 2],
}

impl Placement {
    fn of(at: DrawAt) -> Self {
        Self {
            rect: at.rect,
            clip: [
                at.clip[0].max(at.rect[0]),
                at.clip[1].max(at.rect[1]),
                at.clip[2].min(at.rect[2]),
                at.clip[3].min(at.rect[3]),
            ],
            screen: [at.screen.x, at.screen.y],
            padding: [0.0; 2],
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct GridScene(Rc<RefCell<Option<GridRenderer>>>);

impl GridScene {
    pub(crate) fn drawing(&self, frame: RenderFrame) -> Drawing {
        Drawing::new(GridDraw {
            scene: self.clone(),
            frame,
        })
    }
}

struct GridDraw {
    scene: GridScene,
    frame: RenderFrame,
}

impl Draw for GridDraw {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let mut held = self.scene.0.borrow_mut();
        let renderer = held.get_or_insert_with(|| GridRenderer::new(device, at.format));
        renderer.prepare(device, queue, &self.frame, at);
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        if let Some(renderer) = self.scene.0.borrow().as_ref() {
            renderer.paint(pass);
        }
    }
}

struct GridRenderer {
    placement_buffer: wgpu::Buffer,
    placement_bind_group: wgpu::BindGroup,
    triangle_pipeline: wgpu::RenderPipeline,
    wire_pipeline: wgpu::RenderPipeline,
    wire_value_bind_group_layout: wgpu::BindGroupLayout,
    wire_value_bind_group: wgpu::BindGroup,
    background_vertex_buffer: Arc<wgpu::Buffer>,
    vertex_buffer: Arc<wgpu::Buffer>,
    wire_vertex_buffer: Arc<wgpu::Buffer>,
    value_vertex_buffer: Arc<wgpu::Buffer>,
    wire_value_texture: wgpu::Texture,
    background_vertex_capacity: usize,
    vertex_capacity: usize,
    wire_vertex_capacity: usize,
    value_vertex_capacity: usize,
    wire_value_size: [u32; 2],
    background_vertex_count: u32,
    vertex_count: u32,
    wire_vertex_count: u32,
    value_vertex_count: u32,
}

impl GridRenderer {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("renderer.wgsl"));
        let placement_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("logic placement bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let placement_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("logic placement buffer"),
            size: std::mem::size_of::<Placement>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let placement_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("logic placement bind group"),
            layout: &placement_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: placement_buffer.as_entire_binding(),
            }],
        });
        let triangle_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("logic triangle pipeline layout"),
                bind_group_layouts: &[Some(&placement_bind_group_layout)],
                immediate_size: 0,
            });
        let wire_value_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("logic wire value bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });
        let wire_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("logic wire pipeline layout"),
            bind_group_layouts: &[
                Some(&placement_bind_group_layout),
                Some(&wire_value_bind_group_layout),
            ],
            immediate_size: 0,
        });
        let triangle_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("logic triangle pipeline"),
            layout: Some(&triangle_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[RenderVertex::layout()],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let wire_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("logic wire pipeline"),
            layout: Some(&wire_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("wire_vertex"),
                compilation_options: Default::default(),
                buffers: &[WireVertex::layout()],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("wire_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vertex_capacity = 1;
        let background_vertex_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("logic background vertex buffer"),
            size: std::mem::size_of::<RenderVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        let vertex_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("logic triangle vertex buffer"),
            size: std::mem::size_of::<RenderVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        let wire_vertex_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("logic wire vertex buffer"),
            size: std::mem::size_of::<WireVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        let value_vertex_buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("logic value vertex buffer"),
            size: std::mem::size_of::<WireVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        let wire_value_size = [1, 1];
        let wire_value_texture = create_wire_value_texture(device, wire_value_size);
        let wire_value_view =
            wire_value_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let wire_value_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("logic wire value bind group"),
            layout: &wire_value_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&wire_value_view),
            }],
        });

        Self {
            placement_buffer,
            placement_bind_group,
            triangle_pipeline,
            wire_pipeline,
            wire_value_bind_group_layout,
            wire_value_bind_group,
            background_vertex_buffer,
            vertex_buffer,
            wire_vertex_buffer,
            value_vertex_buffer,
            wire_value_texture,
            background_vertex_capacity: vertex_capacity,
            vertex_capacity,
            wire_vertex_capacity: vertex_capacity,
            value_vertex_capacity: vertex_capacity,
            wire_value_size,
            background_vertex_count: 0,
            vertex_count: 0,
            wire_vertex_count: 0,
            value_vertex_count: 0,
        }
    }

    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &RenderFrame,
        at: DrawAt,
    ) {
        queue.write_buffer(
            &self.placement_buffer,
            0,
            bytemuck::bytes_of(&Placement::of(at)),
        );
        let background_vertices = triangle_vertices(background_triangles(frame), frame);
        prepare_vertex_buffer(
            device,
            queue,
            "logic background vertex buffer",
            &background_vertices,
            &mut self.background_vertex_buffer,
            &mut self.background_vertex_capacity,
        );
        self.background_vertex_count = background_vertices.len() as u32;

        let vertices = triangle_vertices(frame.triangles.clone(), frame);
        prepare_vertex_buffer(
            device,
            queue,
            "logic triangle vertex buffer",
            &vertices,
            &mut self.vertex_buffer,
            &mut self.vertex_capacity,
        );
        self.vertex_count = vertices.len() as u32;

        let mut wire_verts = wire_vertices(&frame.wires, frame);
        wire_verts.extend(ray_vertices(&frame.rays, frame));
        wire_verts.extend(stub_vertices(&frame.stubs, frame));
        prepare_vertex_buffer(
            device,
            queue,
            "logic wire vertex buffer",
            &wire_verts,
            &mut self.wire_vertex_buffer,
            &mut self.wire_vertex_capacity,
        );
        self.wire_vertex_count = wire_verts.len() as u32;

        let value_verts = value_triangle_vertices(&frame.value_triangles, frame);
        prepare_vertex_buffer(
            device,
            queue,
            "logic value vertex buffer",
            &value_verts,
            &mut self.value_vertex_buffer,
            &mut self.value_vertex_capacity,
        );
        self.value_vertex_count = value_verts.len() as u32;

        let mut wire_values = if frame.wire_values.is_empty() {
            vec![WireValue::new(0)]
        } else {
            frame.wire_values.clone()
        };
        let max_width = device.limits().max_texture_dimension_2d;
        let width = (wire_values.len() as u32).min(max_width).max(1);
        let height = (wire_values.len() as u32).div_ceil(width);
        wire_values.resize((width * height) as usize, WireValue::new(0));
        let size = [width, height];
        if size != self.wire_value_size {
            self.wire_value_size = size;
            self.wire_value_texture = create_wire_value_texture(device, size);
            let view = self
                .wire_value_texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            self.wire_value_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("logic wire value bind group"),
                layout: &self.wire_value_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            });
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.wire_value_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&wire_values),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * std::mem::size_of::<WireValue>() as u32),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'_>) {
        render_pass.set_bind_group(0, &self.placement_bind_group, &[]);
        if self.background_vertex_count > 0 {
            render_pass.set_pipeline(&self.triangle_pipeline);
            render_pass.set_vertex_buffer(0, self.background_vertex_buffer.slice(..));
            render_pass.draw(0..self.background_vertex_count, 0..1);
        }
        if self.wire_vertex_count > 0 {
            render_pass.set_pipeline(&self.wire_pipeline);
            render_pass.set_bind_group(1, &self.wire_value_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.wire_vertex_buffer.slice(..));
            render_pass.draw(0..self.wire_vertex_count, 0..1);
        }
        if self.vertex_count > 0 {
            render_pass.set_pipeline(&self.triangle_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..self.vertex_count, 0..1);
        }
        if self.value_vertex_count > 0 {
            render_pass.set_pipeline(&self.wire_pipeline);
            render_pass.set_bind_group(1, &self.wire_value_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.value_vertex_buffer.slice(..));
            render_pass.draw(0..self.value_vertex_count, 0..1);
        }
    }
}

fn create_wire_value_texture(device: &wgpu::Device, size: [u32; 2]) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("logic wire value texture"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rg32Uint,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn prepare_vertex_buffer<T: Pod>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &'static str,
    vertices: &[T],
    buffer: &mut Arc<wgpu::Buffer>,
    capacity: &mut usize,
) {
    if vertices.len() > *capacity {
        *capacity = vertices.len().next_power_of_two();
        *buffer = Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (*capacity * std::mem::size_of::<T>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }
    if !vertices.is_empty() {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(vertices));
    }
}
