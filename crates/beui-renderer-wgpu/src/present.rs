use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::Renderer;
use crate::Repaint;
use crate::Repainting;
use crate::clear_color_in;
use crate::renderer_info;
use beui_core::color::Color32;
use beui_core::context::{FrameOutput, Moved, RendererInfo};
use beui_core::geometry::{Vec2, vec2};

pub type OpenDevice = Arc<
    dyn Fn(&wgpu::Adapter, &wgpu::DeviceDescriptor<'_>) -> Option<(wgpu::Device, wgpu::Queue)>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct GpuSetup {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
}

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
    pub renderer: Renderer,
    open_device: Option<OpenDevice>,
    lost: Arc<AtomicBool>,
}

struct Opened {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    lost: Arc<AtomicBool>,
}

async fn open(
    instance: &wgpu::Instance,
    probe: Option<&wgpu::Surface<'_>>,
    open_device: Option<&OpenDevice>,
) -> Result<Opened, Box<dyn Error>> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: probe,
        })
        .await?;
    let descriptor = wgpu::DeviceDescriptor {
        label: Some("beui device"),
        required_features: wgpu::Features::empty(),
        required_limits: adapter.limits(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    };
    let opened = open_device.and_then(|open| open(&adapter, &descriptor));
    let (device, queue) = match opened {
        Some(opened) => opened,
        None => adapter.request_device(&descriptor).await?,
    };
    let lost = Arc::new(AtomicBool::new(false));
    let flag = lost.clone();
    device.set_device_lost_callback(move |reason, message| {
        if reason != wgpu::DeviceLostReason::Destroyed {
            eprintln!("beui: the GPU was lost: {message}");
        }
        flag.store(true, Ordering::SeqCst);
    });
    let flag = lost.clone();
    device.on_uncaptured_error(Arc::new(move |error| {
        if !flag.load(Ordering::SeqCst) {
            panic!("wgpu error: {error}");
        }
    }));
    Ok(Opened {
        adapter,
        device,
        queue,
        lost,
    })
}

pub async fn create_offscreen(
    format: wgpu::TextureFormat,
    open_device: Option<OpenDevice>,
) -> Result<Gpu, Box<dyn Error>> {
    let instance = wgpu::Instance::default();
    let opened = open(&instance, None, open_device.as_ref()).await?;
    Ok(Gpu::assemble(instance, opened, format, open_device))
}

pub async fn create_gpu(
    instance: wgpu::Instance,
    probe: &wgpu::Surface<'_>,
    open_device: Option<OpenDevice>,
) -> Result<Gpu, Box<dyn Error>> {
    let opened = open(&instance, Some(probe), open_device.as_ref()).await?;
    let capabilities = probe.get_capabilities(&opened.adapter);
    let format =
        surface_format(&capabilities.formats).ok_or("the adapter does not support this surface")?;
    Ok(Gpu::assemble(instance, opened, format, open_device))
}

impl Gpu {
    fn assemble(
        instance: wgpu::Instance,
        opened: Opened,
        format: wgpu::TextureFormat,
        open_device: Option<OpenDevice>,
    ) -> Self {
        Self {
            renderer: Renderer::new(&opened.device, format),
            instance,
            adapter: opened.adapter,
            device: opened.device,
            queue: opened.queue,
            format,
            open_device,
            lost: opened.lost,
        }
    }

    pub fn lost(&self) -> bool {
        self.lost.load(Ordering::SeqCst)
    }

    pub async fn reopen(
        &mut self,
        probe: Option<&wgpu::Surface<'_>>,
    ) -> Result<(), Box<dyn Error>> {
        let opened = open(&self.instance, probe, self.open_device.as_ref()).await?;
        let format = match probe {
            Some(probe) => surface_format(&probe.get_capabilities(&opened.adapter).formats)
                .ok_or("the adapter does not support this surface")?,
            None => self.format,
        };
        let instance = self.instance.clone();
        let open_device = self.open_device.take();
        *self = Self::assemble(instance, opened, format, open_device);
        Ok(())
    }

    pub fn info(&self) -> RendererInfo {
        renderer_info(&self.adapter.get_info(), self.format)
    }

    pub fn setup(&self) -> GpuSetup {
        GpuSetup {
            device: self.device.clone(),
            queue: self.queue.clone(),
            format: self.format,
        }
    }
}

pub fn surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    formats
        .iter()
        .copied()
        .find(|format| !format.is_srgb())
        .or_else(|| formats.first().copied())
}

pub enum Presented {
    Done,
    Again,
}

pub struct Target {
    surface: Option<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,
    prepared_size: Option<(Vec2, f32)>,
    clear_color: Option<Color32>,
    pending: Option<Repaint>,
    moved: Option<(Moved, f32, Repaint)>,
    retained: Option<Retained>,
    presented: bool,
    offscreen: bool,
}

struct Retained {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
}

impl Target {
    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            surface: None,
            config: wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: 0,
                height: 0,
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: Vec::new(),
            },
            prepared_size: None,
            clear_color: None,
            pending: None,
            moved: None,
            retained: None,
            presented: false,
            offscreen: false,
        }
    }

    pub fn offscreen(gpu: &Gpu, width: u32, height: u32) -> Self {
        let mut target = Self::new(gpu.format);
        target.offscreen = true;
        target.config.usage |= wgpu::TextureUsages::COPY_DST;
        target.config.width = width;
        target.config.height = height;
        target
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn capture(&self, gpu: &Gpu) -> Option<beui_core::app::automation::Capture> {
        let retained = self.retained.as_ref().filter(|_| self.presented)?;
        read_texture(&gpu.device, &gpu.queue, &retained.texture)
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn attached(&self) -> bool {
        self.surface.is_some() || self.offscreen
    }

    pub fn physical(&self) -> Option<Vec2> {
        if !self.attached() || self.config.width == 0 || self.config.height == 0 {
            return None;
        }
        Some(vec2(self.config.width as f32, self.config.height as f32))
    }

    pub fn attach(
        &mut self,
        gpu: &Gpu,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<(), Box<dyn Error>> {
        let mut config = surface
            .get_default_config(&gpu.adapter, width.max(1), height.max(1))
            .ok_or("the adapter does not support this surface")?;
        config.format = gpu.format;
        let capabilities = surface.get_capabilities(&gpu.adapter);
        if capabilities.usages.contains(wgpu::TextureUsages::COPY_DST) {
            config.usage |= wgpu::TextureUsages::COPY_DST;
        }
        config.width = width;
        config.height = height;
        self.config = config;
        self.surface = Some(surface);
        self.retained = None;
        self.prepared_size = None;
        self.configure(&gpu.device);
        Ok(())
    }

    pub fn detach(&mut self) {
        self.surface = None;
        self.retained = None;
        self.pending = None;
        self.moved = None;
    }

    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        self.configure(&gpu.device);
    }

    fn configure(&mut self, device: &wgpu::Device) {
        if self.config.width == 0 || self.config.height == 0 {
            return;
        }
        if let Some(surface) = &self.surface {
            surface.configure(device, &self.config);
        }
    }

    fn retain(&mut self, device: &wgpu::Device) {
        if !self.config.usage.contains(wgpu::TextureUsages::COPY_DST) {
            return;
        }
        let size = (self.config.width, self.config.height);
        if self
            .retained
            .as_ref()
            .is_none_or(|retained| retained.size != size)
        {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("beui retained frame"),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.retained = Some(Retained {
                texture,
                view,
                size,
            });
        }
    }

    fn retains(&self) -> bool {
        self.retained
            .as_ref()
            .is_some_and(|retained| retained.size == (self.config.width, self.config.height))
    }

    pub fn prepare(
        &mut self,
        gpu: &mut Gpu,
        output: &FrameOutput,
        scale: f32,
        clear_color: Color32,
    ) -> bool {
        let physical = vec2(self.config.width as f32, self.config.height as f32);
        let size = (physical, scale);
        let stale = self.prepared_size != Some(size)
            || self.clear_color != Some(clear_color)
            || !self.retains();
        let (repaint, moved, whole) = match stale {
            true => (Repaint::Everything, None, Repaint::Everything),
            false => {
                let (repaint, moved) = output.repaint_moving(clear_color);
                (repaint, moved, output.repaint(clear_color))
            }
        };
        if output.changed || stale {
            let (repaint, moved) = match self.pending {
                Some(pending) => {
                    let held = self.moved.take().map_or(pending, |(_, _, whole)| whole);
                    (held.union(whole), None)
                }
                None => (repaint, moved),
            };
            let effective =
                gpu.renderer
                    .prepare(&gpu.device, &gpu.queue, output, physical, scale, repaint);
            self.moved = match effective {
                Repaint::Region { .. } => moved.map(|moved| (moved, scale, whole)),
                Repaint::Everything => None,
            };
            self.prepared_size = Some(size);
            self.pending = Some(effective);
        }
        self.clear_color = Some(clear_color);
        self.pending.is_some()
    }

    pub fn present(&mut self, gpu: &mut Gpu, background: Color32) -> Presented {
        if gpu.lost() {
            return Presented::Again;
        }
        if self.config.width == 0 || self.config.height == 0 {
            return Presented::Done;
        }
        let frame = match &self.surface {
            Some(surface) => match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Some(frame),
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    surface.configure(&gpu.device, &self.config);
                    return Presented::Again;
                }
                _ => return Presented::Done,
            },
            None if self.offscreen => None,
            None => return Presented::Done,
        };
        let view = frame.as_ref().map(|frame| {
            frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default())
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beui encoder"),
            });
        let clear = clear_color_in(self.config.format, background);
        self.retain(&gpu.device);
        let pending = self.pending.take();
        let moved = self.moved.take();
        let size = (self.config.width, self.config.height);
        let retained = self.retained.as_ref();
        if let (Some((moved, scale, _)), Some(retained), Some(Repaint::Region { .. })) =
            (moved, retained, pending)
        {
            gpu.renderer
                .shift(&gpu.device, &mut encoder, &retained.texture, moved, scale);
        }
        let (target, load) = match retained {
            Some(retained) => (
                &retained.view,
                match pending {
                    Some(Repaint::Region { .. }) => wgpu::LoadOp::Load,
                    _ => wgpu::LoadOp::Clear(clear),
                },
            ),
            None => match &view {
                Some(view) => (view, wgpu::LoadOp::Clear(clear)),
                None => return Presented::Done,
            },
        };
        if pending.is_some() || retained.is_none() {
            gpu.renderer
                .render(&gpu.device, &gpu.queue, &mut encoder, target, size, load);
        }
        if let (Some(retained), Some(frame)) = (retained, &frame) {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &retained.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &frame.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: retained.size.0,
                    height: retained.size.1,
                    depth_or_array_layers: 1,
                },
            );
        }
        gpu.queue.submit(Some(encoder.finish()));
        if let Some(frame) = frame {
            frame.present();
        }
        self.presented = true;
        match gpu.lost() {
            true => Presented::Again,
            false => Presented::Done,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Option<beui_core::app::automation::Capture> {
    let swap = match texture.format() {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => false,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => true,
        _ => return None,
    };
    let (width, height) = (texture.width(), texture.height());
    let row = width * 4;
    let padded =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("beui capture"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("beui capture encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
    let mapped = buffer.slice(..).get_mapped_range();
    let mut rgba = Vec::with_capacity((row * height) as usize);
    for line in mapped.chunks(padded as usize) {
        rgba.extend_from_slice(&line[..row as usize]);
    }
    drop(mapped);
    if swap {
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
    }
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    Some(beui_core::app::automation::Capture {
        width,
        height,
        rgba,
    })
}
