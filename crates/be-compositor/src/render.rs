use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use beui::{Draw, DrawAt, Drawing, Waker};
use bytemuck::{Pod, Zeroable};
use smithay::backend::allocator::dmabuf::{Dmabuf, WeakDmabuf};
use smithay::backend::renderer::utils::{Buffer, with_renderer_surface_state};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::shm::with_buffer_contents;

use crate::gpu::{Usage, Vulkan};
use crate::state::Layer;

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    srgb: bool,
    vulkan: Option<Vulkan>,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

impl Gpu {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("render.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("client surface"),
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
            label: Some("client surface"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("client surface"),
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
            label: Some("client surface"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let vulkan = Vulkan::of(&device);
        Self {
            device,
            queue,
            srgb: format.is_srgb(),
            vulkan,
            pipeline,
            layout,
            sampler,
        }
    }
}

pub struct SurfaceTexture {
    texture: wgpu::Texture,
    group: wgpu::BindGroup,
    opaque: bool,
}

impl SurfaceTexture {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}

#[derive(Clone)]
pub struct Current {
    pub texture: Rc<SurfaceTexture>,
    pub buffer: Option<Buffer>,
}

#[derive(Default)]
pub struct Textures {
    gpu: Option<Rc<Gpu>>,
    surfaces: HashMap<WlSurface, Current>,
    dmabufs: HashMap<WeakDmabuf, Rc<SurfaceTexture>>,
}

impl Textures {
    pub fn set_gpu(&mut self, gpu: Gpu) {
        self.gpu = Some(Rc::new(gpu));
    }

    pub fn gpu(&self) -> Option<Rc<Gpu>> {
        self.gpu.clone()
    }

    pub fn get(&self, surface: &WlSurface) -> Option<Current> {
        self.surfaces.get(surface).cloned()
    }

    pub fn prune(&mut self) {
        self.surfaces.retain(|surface, _| surface.is_alive());
        self.dmabufs.retain(|dmabuf, _| !dmabuf.is_gone());
    }

    pub fn dmabuf_formats(
        &self,
    ) -> Option<(Vec<smithay::backend::allocator::Format>, Option<u64>)> {
        let vulkan = self.gpu.as_ref()?.vulkan.as_ref()?;
        Some((vulkan.formats(Usage::Sample), vulkan.render_node()))
    }

    pub fn import(&mut self, dmabuf: &Dmabuf) -> Option<Rc<SurfaceTexture>> {
        if let Some(texture) = self.dmabufs.get(&dmabuf.weak()) {
            return Some(texture.clone());
        }
        let gpu = self.gpu.clone()?;
        let vulkan = gpu.vulkan.as_ref()?;
        let imported = vulkan
            .import(&gpu.device, dmabuf, gpu.srgb, Usage::Sample)
            .inspect_err(|error| {
                eprintln!("be-compositor: a client buffer did not import: {error}")
            })
            .ok()?;
        let texture = Rc::new(gpu.bind(imported.texture, imported.opaque));
        self.dmabufs.insert(dmabuf.weak(), texture.clone());
        Some(texture)
    }

    pub fn upload(&mut self, surface: &WlSurface) {
        let Some(gpu) = self.gpu.clone() else {
            return;
        };
        let buffer =
            with_renderer_surface_state(surface, |state| state.buffer().cloned()).flatten();
        let Some(buffer) = buffer else {
            self.surfaces.remove(surface);
            return;
        };
        if let Ok(dmabuf) = get_dmabuf(&buffer).cloned() {
            match self.import(&dmabuf) {
                Some(texture) => {
                    self.surfaces.insert(
                        surface.clone(),
                        Current {
                            texture,
                            buffer: Some(buffer),
                        },
                    );
                }
                None => {
                    self.surfaces.remove(surface);
                }
            }
            return;
        }
        let previous = self
            .surfaces
            .get(surface)
            .filter(|current| current.buffer.is_none())
            .map(|current| current.texture.clone());
        let uploaded = with_buffer_contents(&buffer, |pointer, length, data| {
            let (format, opaque) = match (data.format, gpu.srgb) {
                (wl_shm::Format::Argb8888, true) => (wgpu::TextureFormat::Bgra8UnormSrgb, false),
                (wl_shm::Format::Argb8888, false) => (wgpu::TextureFormat::Bgra8Unorm, false),
                (wl_shm::Format::Xrgb8888, true) => (wgpu::TextureFormat::Bgra8UnormSrgb, true),
                (wl_shm::Format::Xrgb8888, false) => (wgpu::TextureFormat::Bgra8Unorm, true),
                (wl_shm::Format::Abgr8888, true) => (wgpu::TextureFormat::Rgba8UnormSrgb, false),
                (wl_shm::Format::Abgr8888, false) => (wgpu::TextureFormat::Rgba8Unorm, false),
                (wl_shm::Format::Xbgr8888, true) => (wgpu::TextureFormat::Rgba8UnormSrgb, true),
                (wl_shm::Format::Xbgr8888, false) => (wgpu::TextureFormat::Rgba8Unorm, true),
                _ => return None,
            };
            let (width, height) = (data.width.max(1) as u32, data.height.max(1) as u32);
            let offset = data.offset.max(0) as usize;
            let needed = data.stride as usize * (height as usize - 1) + width as usize * 4;
            if data.stride < data.width * 4 || offset + needed > length {
                return None;
            }
            let bytes = unsafe { std::slice::from_raw_parts(pointer.add(offset), needed) };
            let size = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };
            let reused = previous.filter(|texture| {
                texture.texture.size() == size
                    && texture.texture.format() == format
                    && texture.opaque == opaque
            });
            let texture = match reused {
                Some(texture) => texture,
                None => Rc::new(gpu.texture(size, format, opaque)),
            };
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(data.stride as u32),
                    rows_per_image: Some(height),
                },
                size,
            );
            Some(texture)
        });
        match uploaded.ok().flatten() {
            Some(texture) => {
                self.surfaces.insert(
                    surface.clone(),
                    Current {
                        texture,
                        buffer: None,
                    },
                );
            }
            None => {
                self.surfaces.remove(surface);
            }
        }
    }
}

impl Gpu {
    pub fn rgba(&self, width: u32, height: u32, pixels: &[u8]) -> SurfaceTexture {
        let format = match self.srgb {
            true => wgpu::TextureFormat::Rgba8UnormSrgb,
            false => wgpu::TextureFormat::Rgba8Unorm,
        };
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.texture(size, format, false);
        self.queue.write_texture(
            texture.texture.as_image_copy(),
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            size,
        );
        texture
    }

    pub fn paint_sprite(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        target_size: (u32, u32),
        rect: [f32; 4],
        texture: &SurfaceTexture,
    ) {
        let (width, height) = (target_size.0 as f32, target_size.1 as f32);
        let corner = |x: f32, y: f32, u: f32, v: f32| Vertex {
            position: [x / width * 2.0 - 1.0, 1.0 - y / height * 2.0],
            uv: [u, v],
            opaque: f32::from(u8::from(texture.opaque)),
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
        pass.set_bind_group(0, &texture.group, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..6, 0..1);
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

    fn texture(
        &self,
        size: wgpu::Extent3d,
        format: wgpu::TextureFormat,
        opaque: bool,
    ) -> SurfaceTexture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("client surface"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.bind(texture, opaque)
    }

    fn bind(&self, texture: wgpu::Texture, opaque: bool) -> SurfaceTexture {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("client surface"),
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
        SurfaceTexture {
            texture,
            group,
            opaque,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    opaque: f32,
}

struct Vertices {
    buffer: wgpu::Buffer,
    capacity: usize,
    quads: Vec<(usize, u32)>,
}

pub struct WindowDraw {
    gpu: Rc<Gpu>,
    layers: Vec<(Rc<SurfaceTexture>, Layer)>,
    buffers: Vec<Buffer>,
    vertices: RefCell<Option<Vertices>>,
    painted: Rc<Cell<bool>>,
    waker: Option<Waker>,
}

impl WindowDraw {
    pub fn drawing(
        gpu: Rc<Gpu>,
        layers: Vec<(Current, Layer)>,
        painted: Rc<Cell<bool>>,
        waker: Option<Waker>,
    ) -> Drawing {
        let buffers = layers
            .iter()
            .filter_map(|(current, _)| current.buffer.clone())
            .collect();
        let layers = layers
            .into_iter()
            .map(|(current, layer)| (current.texture, layer))
            .collect();
        Drawing::new(Self {
            gpu,
            layers,
            buffers,
            vertices: RefCell::new(None),
            painted,
            waker,
        })
    }
}

fn quad(layer: &Layer, at: &DrawAt) -> Option<[Vertex; 6]> {
    let scale = at.pixels_per_point;
    let left = at.rect[0] + layer.rect.loc.x as f32 * scale;
    let top = at.rect[1] + layer.rect.loc.y as f32 * scale;
    let right = left + layer.rect.size.w as f32 * scale;
    let bottom = top + layer.rect.size.h as f32 * scale;
    let clip = [
        at.clip[0].max(at.rect[0]),
        at.clip[1].max(at.rect[1]),
        at.clip[2].min(at.rect[2]),
        at.clip[3].min(at.rect[3]),
    ];
    let (x0, y0) = (left.max(clip[0]), top.max(clip[1]));
    let (x1, y1) = (right.min(clip[2]), bottom.min(clip[3]));
    if x1 <= x0 || y1 <= y0 || layer.size.w <= 0.0 || layer.size.h <= 0.0 {
        return None;
    }
    let u0 = (layer.source.loc.x / layer.size.w) as f32;
    let v0 = (layer.source.loc.y / layer.size.h) as f32;
    let u1 = ((layer.source.loc.x + layer.source.size.w) / layer.size.w) as f32;
    let v1 = ((layer.source.loc.y + layer.source.size.h) / layer.size.h) as f32;
    let u = |x: f32| u0 + (x - left) / (right - left) * (u1 - u0);
    let v = |y: f32| v0 + (y - top) / (bottom - top) * (v1 - v0);
    let ndc_x = |x: f32| x / at.screen.x * 2.0 - 1.0;
    let ndc_y = |y: f32| 1.0 - y / at.screen.y * 2.0;
    let corner = |x: f32, y: f32| Vertex {
        position: [ndc_x(x), ndc_y(y)],
        uv: [u(x), v(y)],
        opaque: 0.0,
    };
    let (a, b, c, d) = (
        corner(x0, y0),
        corner(x1, y0),
        corner(x1, y1),
        corner(x0, y1),
    );
    Some([a, b, c, a, c, d])
}

impl Draw for WindowDraw {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let mut vertices = Vec::new();
        let mut quads = Vec::new();
        for (index, (texture, layer)) in self.layers.iter().enumerate() {
            if let Some(mut corners) = quad(layer, &at) {
                for corner in &mut corners {
                    corner.opaque = f32::from(u8::from(texture.opaque));
                }
                quads.push((index, vertices.len() as u32));
                vertices.extend_from_slice(&corners);
            }
        }
        let mut held = self.vertices.borrow_mut();
        let needed = vertices.len().max(6);
        if held.as_ref().is_none_or(|held| held.capacity < needed) {
            *held = Some(Vertices {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("client surface vertices"),
                    size: (needed.next_power_of_two() * std::mem::size_of::<Vertex>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                capacity: needed.next_power_of_two(),
                quads: Vec::new(),
            });
        }
        let held = held.as_mut().expect("the vertex buffer was just made");
        if !vertices.is_empty() {
            queue.write_buffer(&held.buffer, 0, bytemuck::cast_slice(&vertices));
        }
        held.quads = quads;
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        if let Some(held) = self.vertices.borrow().as_ref()
            && !held.quads.is_empty()
        {
            pass.set_pipeline(&self.gpu.pipeline);
            pass.set_vertex_buffer(0, held.buffer.slice(..));
            for (index, first) in &held.quads {
                pass.set_bind_group(0, &self.layers[*index].0.group, &[]);
                pass.draw(*first..*first + 6, 0..1);
            }
        }
        self.painted.set(true);
        if let Some(waker) = &self.waker {
            waker.wake();
        }
    }
}

impl Drop for WindowDraw {
    fn drop(&mut self) {
        let buffers = std::mem::take(&mut self.buffers);
        if !buffers.is_empty() {
            self.gpu.queue.on_submitted_work_done(move || drop(buffers));
        }
    }
}
