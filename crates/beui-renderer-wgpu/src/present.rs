use std::error::Error;
use std::ffi::CStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::Renderer;
use crate::Repaint;
use crate::Repainting;
use crate::renderer_info;
use crate::wgpu_color_in;
use beui_core::color::Color32;
use beui_core::context::{Moved, RendererInfo, WindowOutput};
use beui_core::geometry::{Vec2, vec2};
use beui_runner::renderer::WindowHandle;

pub trait OpenGpu: Send + Sync {
    fn adapter(&self) -> Result<(wgpu::Instance, Option<wgpu::Adapter>), Box<dyn Error>>;

    fn device(
        &self,
        adapter: &wgpu::Adapter,
        extensions: &[&'static CStr],
        descriptor: &wgpu::DeviceDescriptor<'_>,
    ) -> Result<Option<(wgpu::Device, wgpu::Queue)>, Box<dyn Error>>;

    fn stale(&self) -> bool {
        false
    }
}

#[derive(Clone, Default)]
pub struct OpenDevice {
    pub extensions: &'static [&'static CStr],
    pub gpu: Option<Arc<dyn OpenGpu>>,
}

impl OpenDevice {
    pub fn instance(&self) -> Result<(wgpu::Instance, Option<wgpu::Adapter>), Box<dyn Error>> {
        match &self.gpu {
            Some(gpu) => gpu.adapter(),
            None => Ok((plain_instance(), None)),
        }
    }

    pub fn stale(&self) -> bool {
        self.gpu.as_ref().is_some_and(|gpu| gpu.stale())
    }

    fn open(
        &self,
        adapter: &wgpu::Adapter,
        descriptor: &wgpu::DeviceDescriptor<'_>,
    ) -> Result<Option<(wgpu::Device, wgpu::Queue)>, Box<dyn Error>> {
        let opened = match &self.gpu {
            Some(gpu) => gpu.device(adapter, self.extensions, descriptor)?,
            None => None,
        };
        Ok(opened.or_else(|| open_vulkan_device(adapter, descriptor, self.extensions)))
    }
}

pub fn plain_instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env())
}

// unsafe(ffi)
#[cfg(any(windows, all(unix, not(target_vendor = "apple"))))]
pub fn open_vulkan_device(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'_>,
    extensions: &[&'static CStr],
) -> Option<(wgpu::Device, wgpu::Queue)> {
    if extensions.is_empty() {
        return None;
    }
    let hal = unsafe { adapter.as_hal::<wgpu::hal::api::Vulkan>() }?;
    let capabilities = hal.physical_device_capabilities();
    let extra: Vec<&'static CStr> = extensions
        .iter()
        .copied()
        .filter(|name| capabilities.supports_extension(name))
        .collect();
    let opened = unsafe {
        hal.open_with_callback(
            descriptor.required_features,
            &descriptor.required_limits,
            &descriptor.memory_hints,
            Some(Box::new(move |arguments| {
                for name in extra {
                    if !arguments.extensions.contains(&name) {
                        arguments.extensions.push(name);
                    }
                }
            })),
        )
    }
    .ok()?;
    drop(hal);
    unsafe { adapter.create_device_from_hal(opened, descriptor) }.ok()
}

#[cfg(not(any(windows, all(unix, not(target_vendor = "apple")))))]
pub fn open_vulkan_device(
    _adapter: &wgpu::Adapter,
    _descriptor: &wgpu::DeviceDescriptor<'_>,
    _extensions: &[&'static CStr],
) -> Option<(wgpu::Device, wgpu::Queue)> {
    None
}

pub fn browser_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    adapter.features() & wgpu::Features::all_webgpu_mask()
}

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
    open_device: OpenDevice,
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
    adapter: Option<wgpu::Adapter>,
    probe: Option<&wgpu::Surface<'_>>,
    open_device: &OpenDevice,
) -> Result<Opened, Box<dyn Error>> {
    let adapter = match adapter {
        Some(adapter) => adapter,
        None => {
            instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    force_fallback_adapter: false,
                    compatible_surface: probe,
                })
                .await?
        }
    };
    let descriptor = wgpu::DeviceDescriptor {
        label: Some("beui device"),
        required_features: browser_features(&adapter),
        required_limits: adapter.limits(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    };
    let (device, queue) = match open_device.open(&adapter, &descriptor)? {
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
    open_device: OpenDevice,
) -> Result<Gpu, Box<dyn Error>> {
    let (instance, adapter) = open_device.instance()?;
    let opened = open(&instance, adapter, None, &open_device).await?;
    Ok(Gpu::assemble(instance, opened, format, open_device))
}

pub async fn create_gpu(
    instance: wgpu::Instance,
    adapter: Option<wgpu::Adapter>,
    probe: &wgpu::Surface<'_>,
    open_device: OpenDevice,
) -> Result<Gpu, Box<dyn Error>> {
    let opened = open(&instance, adapter, Some(probe), &open_device).await?;
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
        open_device: OpenDevice,
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

    pub fn stale(&self) -> bool {
        self.lost() || self.open_device.stale()
    }

    pub async fn reopen(
        &mut self,
        window: Option<Arc<dyn WindowHandle>>,
    ) -> Result<Option<wgpu::Surface<'static>>, Box<dyn Error>> {
        let (instance, adapter) = match self.open_device.gpu {
            Some(_) => self.open_device.instance()?,
            None => (self.instance.clone(), None),
        };
        let probe = match window {
            Some(window) => Some(instance.create_surface(window)?),
            None => None,
        };
        let opened = open(&instance, adapter, probe.as_ref(), &self.open_device).await?;
        let format = match &probe {
            Some(probe) => surface_format(&probe.get_capabilities(&opened.adapter).formats)
                .ok_or("the adapter does not support this surface")?,
            None => self.format,
        };
        let open_device = std::mem::take(&mut self.open_device);
        *self = Self::assemble(instance, opened, format, open_device);
        Ok(probe)
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
    let supported = || {
        formats
            .iter()
            .copied()
            .filter(|format| format.required_features().is_empty())
    };
    supported()
        .find(|format| {
            matches!(
                format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
            )
        })
        .or_else(|| supported().find(|format| !format.is_srgb()))
        .or_else(|| supported().next())
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
    unpainted: Option<Unpainted>,
    retained: Option<Retained>,
    presented: bool,
    offscreen: bool,
}

struct Unpainted {
    repaint: Repaint,
    moved: Option<(Moved, f32)>,
    whole: Repaint,
    encoded: bool,
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
            unpainted: None,
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
    pub fn capture(&self, gpu: &Gpu) -> Option<beui_runner::renderer::Capture> {
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
        config.present_mode = self.config.present_mode;
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
        self.unpainted = None;
    }

    pub fn set_vsync(&mut self, gpu: &Gpu, vsync: bool) {
        self.config.present_mode = match vsync {
            true => wgpu::PresentMode::Fifo,
            false => wgpu::PresentMode::AutoNoVsync,
        };
        self.configure(&gpu.device);
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

    pub fn note(&mut self, output: &WindowOutput, scale: f32, clear_color: Color32) {
        let physical = vec2(self.config.width as f32, self.config.height as f32);
        let size = (physical, scale);
        let stale = self.prepared_size != Some(size)
            || self.clear_color != Some(clear_color)
            || !self.retains();
        self.clear_color = Some(clear_color);
        if !(output.changed || stale) {
            return;
        }
        self.prepared_size = Some(size);
        let whole = match stale {
            true => Repaint::Everything,
            false => output.repaint(clear_color),
        };
        self.unpainted = Some(match self.unpainted.take() {
            Some(held) => {
                let held = match held.moved {
                    Some(_) => held.whole,
                    None => held.repaint,
                };
                let repaint = held.union(whole);
                Unpainted {
                    repaint,
                    moved: None,
                    whole: repaint,
                    encoded: false,
                }
            }
            None if stale => Unpainted {
                repaint: Repaint::Everything,
                moved: None,
                whole,
                encoded: false,
            },
            None => {
                let (repaint, moved) = output.repaint_moving(clear_color);
                Unpainted {
                    repaint,
                    moved: moved.map(|moved| (moved, scale)),
                    whole,
                    encoded: false,
                }
            }
        });
    }

    pub fn prepare(
        &mut self,
        gpu: &mut Gpu,
        output: &WindowOutput,
        scale: f32,
        clear_color: Color32,
    ) -> bool {
        self.note(output, scale, clear_color);
        let Some(unpainted) = &mut self.unpainted else {
            return false;
        };
        if !unpainted.encoded {
            let physical = vec2(self.config.width as f32, self.config.height as f32);
            let effective = gpu.renderer.prepare(
                &gpu.device,
                &gpu.queue,
                output,
                physical,
                scale,
                unpainted.repaint,
            );
            if matches!(effective, Repaint::Everything) {
                unpainted.moved = None;
            }
            unpainted.repaint = effective;
            unpainted.encoded = true;
        }
        true
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
        let clear = wgpu_color_in(self.config.format, background);
        self.retain(&gpu.device);
        let (pending, moved) = match self.unpainted.take_if(|unpainted| unpainted.encoded) {
            Some(unpainted) => (Some(unpainted.repaint), unpainted.moved),
            None => (None, None),
        };
        let size = (self.config.width, self.config.height);
        let retained = self.retained.as_ref();
        if let (Some((moved, scale)), Some(retained), Some(Repaint::Region { .. })) =
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
) -> Option<beui_runner::renderer::Capture> {
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
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
    receiver.try_recv().ok()?.ok()?;
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
    Some(beui_runner::renderer::Capture {
        width,
        height,
        rgba,
    })
}
