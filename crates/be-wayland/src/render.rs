use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use beui::native::painting::{
    Content, Fade, Frame, Painting, Paints, Pixels, Primitive, Triangle, Vertex as PaintVertex,
};
use beui::native::texture_quad::{Bound, Pipelines, Quad, Quads};
use beui::{Rect, native::Draw, native::DrawAt, native::Drawing, native::Waker};
use smithay::backend::allocator::dmabuf::{Dmabuf, WeakDmabuf};
use smithay::backend::renderer::utils::{Buffer, with_renderer_surface_state};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::shm::with_buffer_contents;

use smithay::utils::{Logical, Rectangle, Size};

use crate::state::Layer;
use be_dmabuf::{Usage, Vulkan};

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    srgb: bool,
    vulkan: Option<Vulkan>,
    quads: Pipelines,
}

impl Gpu {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let vulkan = Vulkan::of(&device);
        Self {
            quads: Pipelines::new(&device),
            device,
            queue,
            srgb: format.is_srgb(),
            vulkan,
        }
    }
}

pub struct SurfaceTexture {
    texture: wgpu::Texture,
    bound: Bound,
    opaque: bool,
}

impl SurfaceTexture {
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn opaque(&self) -> bool {
        self.opaque
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
        self.surfaces.clear();
        self.dmabufs.clear();
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
            .inspect_err(|error| eprintln!("be-wayland: a client buffer did not import: {error}"))
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
        if let Some((rgba, opaque)) = crate::single_pixel::color(&buffer) {
            let format = match gpu.srgb {
                true => wgpu::TextureFormat::Rgba8UnormSrgb,
                false => wgpu::TextureFormat::Rgba8Unorm,
            };
            let size = wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            };
            let texture = previous
                .filter(|texture| {
                    texture.texture.size() == size
                        && texture.texture.format() == format
                        && texture.opaque == opaque
                })
                .unwrap_or_else(|| Rc::new(gpu.texture(size, format, opaque)));
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                size,
            );
            self.surfaces.insert(
                surface.clone(),
                Current {
                    texture,
                    buffer: None,
                },
            );
            return;
        }
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
            // unsafe(ffi)
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
        SurfaceTexture {
            bound: self.quads.bind(&texture),
            texture,
            opaque,
        }
    }
}

pub struct WindowDraw {
    gpu: Rc<Gpu>,
    layers: Vec<(Rc<SurfaceTexture>, Layer)>,
    buffers: Vec<Buffer>,
    quads: Quads,
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
        beui::native::painted_drawing(Self {
            gpu,
            layers,
            buffers,
            quads: Quads::default(),
            painted,
            waker,
        })
    }
}

fn place(
    rect: Rectangle<f64, Logical>,
    source: Rectangle<f64, Logical>,
    buffer: Size<f64, Logical>,
    at: &DrawAt,
) -> Quad {
    let scale = at.pixels_per_point;
    let left = at.rect[0] + rect.loc.x as f32 * scale;
    let top = at.rect[1] + rect.loc.y as f32 * scale;
    let (width, height) = (
        buffer.w.max(f64::MIN_POSITIVE),
        buffer.h.max(f64::MIN_POSITIVE),
    );
    Quad {
        rect: [
            left,
            top,
            left + rect.size.w as f32 * scale,
            top + rect.size.h as f32 * scale,
        ],
        uv: [
            (source.loc.x / width) as f32,
            (source.loc.y / height) as f32,
            ((source.loc.x + source.size.w) / width) as f32,
            ((source.loc.y + source.size.h) / height) as f32,
        ],
        clip: [
            at.clip[0].max(at.rect[0]),
            at.clip[1].max(at.rect[1]),
            at.clip[2].min(at.rect[2]),
            at.clip[3].min(at.rect[3]),
        ],
        ..Quad::default()
    }
}

impl Draw for WindowDraw {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let quads = self
            .layers
            .iter()
            .filter(|(_, layer)| layer.buffer.w > 0.0 && layer.buffer.h > 0.0)
            .map(|(texture, layer)| {
                let quad = Quad {
                    opaque: texture.opaque,
                    ..place(layer.rect, layer.source, layer.buffer, &at)
                };
                (&texture.bound, quad)
            });
        self.quads.prepare(
            &self.gpu.quads,
            queue,
            at.format,
            [at.screen.x, at.screen.y],
            quads,
        );
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        self.quads.paint(pass);
        self.painted.set(true);
        if let Some(waker) = &self.waker {
            waker.wake();
        }
    }
}

impl Paints for WindowDraw {
    fn painting(&self, rect: Rect) -> Option<Painting> {
        let mut textures = BTreeMap::new();
        let mut primitives = Vec::new();
        for (texture, layer) in &self.layers {
            if !texture
                .texture
                .usage()
                .contains(wgpu::TextureUsages::COPY_SRC)
            {
                continue;
            }
            let Some(capture) = beui_host::native::read_texture(
                &self.gpu.device,
                &self.gpu.queue,
                &texture.texture,
            ) else {
                continue;
            };
            let key = std::ptr::from_ref(texture.as_ref()) as u64;
            textures.insert(
                key,
                Pixels {
                    size: [capture.width, capture.height],
                    rgba: capture.rgba.as_chunks::<4>().0.to_vec(),
                },
            );
            let left = rect.min.x + layer.rect.loc.x as f32;
            let top = rect.min.y + layer.rect.loc.y as f32;
            let right = left + layer.rect.size.w as f32;
            let bottom = top + layer.rect.size.h as f32;
            let (width, height) = (layer.buffer.w.max(1.0), layer.buffer.h.max(1.0));
            let u0 = (layer.source.loc.x / width) as f32;
            let v0 = (layer.source.loc.y / height) as f32;
            let u1 = ((layer.source.loc.x + layer.source.size.w) / width) as f32;
            let v1 = ((layer.source.loc.y + layer.source.size.h) / height) as f32;
            let corner = |x: f32, y: f32, u: f32, v: f32| PaintVertex {
                pos: [x, y],
                uv: [u, v],
                color: [255; 4],
            };
            let (a, b, c, d) = (
                corner(left, top, u0, v0),
                corner(right, top, u1, v0),
                corner(right, bottom, u1, v1),
                corner(left, bottom, u0, v1),
            );
            primitives.push(Primitive {
                clip: [rect.min.x, rect.min.y, rect.max.x, rect.max.y],
                fade: Fade::NONE,
                rounds: Vec::new(),
                content: Content::Mesh(vec![
                    Triangle {
                        texture: key,
                        corners: [a, b, c],
                    },
                    Triangle {
                        texture: key,
                        corners: [a, c, d],
                    },
                ]),
            });
        }
        Some(Painting {
            frame: Frame {
                size: [1, 1],
                pixels_per_point: 1.0,
                background: [0; 4],
                primitives,
            },
            textures,
        })
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

#[cfg(test)]
mod tests;
