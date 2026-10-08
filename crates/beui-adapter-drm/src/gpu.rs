use std::cell::RefCell;
use std::os::fd::OwnedFd;
use std::rc::Rc;

use be_dmabuf::{SyncFiles, Vulkan};
use beui::Vec2;
use bytemuck::{Pod, Zeroable};

pub struct CursorImage {
    pub texture: wgpu::Texture,
    pub size: Vec2,
    pub hotspot: Vec2,
    pub opaque: bool,
}

#[derive(Clone, Default)]
pub struct SoftwareCursor(Rc<RefCell<Option<CursorImage>>>);

impl SoftwareCursor {
    pub fn set(&self, image: Option<CursorImage>) {
        *self.0.borrow_mut() = image;
    }

    pub fn shown(&self) -> bool {
        self.0.borrow().is_some()
    }

    pub(crate) fn with<R>(&self, read: impl FnOnce(Option<&CursorImage>) -> R) -> R {
        read(self.0.borrow().as_ref())
    }
}

pub struct Sprite {
    group: wgpu::BindGroup,
    opaque: bool,
}

pub enum Submitted {
    Fence(Option<OwnedFd>),
    Index(wgpu::SubmissionIndex),
}

pub struct Gpu {
    sync: RefCell<Option<SyncFiles>>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    vulkan: Option<Vulkan>,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    opaque: f32,
}

impl Gpu {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("sprite.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("surface_vertex"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &ATTRIBUTES,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("surface_fragment"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let vulkan = Vulkan::of(&device);
        let sync = vulkan.as_ref().and_then(Vulkan::sync_files);
        if sync.is_none() {
            eprintln!("beui: the GPU cannot export fences, so each frame is waited for");
        }
        Self {
            sync: RefCell::new(sync),
            device,
            queue,
            vulkan,
            pipeline,
            layout,
            sampler,
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn vulkan(&self) -> Option<&Vulkan> {
        self.vulkan.as_ref()
    }

    pub fn submit(&self, commands: wgpu::CommandBuffer) -> Submitted {
        let mut sync = self.sync.borrow_mut();
        let Some(files) = sync.as_ref() else {
            return Submitted::Index(self.queue.submit([commands]));
        };
        match files.submit(&self.queue, commands) {
            Ok(fence) => Submitted::Fence(fence),
            Err(error) => {
                eprintln!("beui: {error}, so each frame is waited for from now on");
                *sync = None;
                Submitted::Index(self.queue.submit([]))
            }
        }
    }

    pub fn rgba(&self, width: u32, height: u32, pixels: &[u8]) -> Sprite {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sprite"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            texture.as_image_copy(),
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            size,
        );
        self.sprite(&texture, false)
    }

    pub fn sprite(&self, texture: &wgpu::Texture, opaque: bool) -> Sprite {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        Sprite { group, opaque }
    }

    pub fn paint_sprite(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        target_size: (u32, u32),
        rect: [f32; 4],
        sprite: &Sprite,
    ) {
        let (width, height) = (target_size.0 as f32, target_size.1 as f32);
        let corner = |x: f32, y: f32, u: f32, v: f32| Vertex {
            position: [x / width * 2.0 - 1.0, 1.0 - y / height * 2.0],
            uv: [u, v],
            opaque: f32::from(u8::from(sprite.opaque)),
        };
        let [left, top, right, bottom] = rect;
        let (a, b, c, d) = (
            corner(left, top, 0.0, 0.0),
            corner(right, top, 1.0, 0.0),
            corner(right, bottom, 1.0, 1.0),
            corner(left, bottom, 0.0, 1.0),
        );
        let vertices = [a, b, c, a, c, d];
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite vertices"),
            size: std::mem::size_of_val(&vertices) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue
            .write_buffer(&buffer, 0, bytemuck::cast_slice(&vertices));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sprite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &sprite.group, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..6, 0..1);
    }
}
