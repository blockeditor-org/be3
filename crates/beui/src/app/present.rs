use std::error::Error;

use crate::color::Color32;
use crate::context::{Context, FrameOutput};
use crate::geometry::{Vec2, vec2};
use crate::renderer::{Renderer, RendererInfo, Repaint, clear_color};

pub(super) struct Gpu {
    pub(super) instance: wgpu::Instance,
    pub(super) adapter: wgpu::Adapter,
    pub(super) device: wgpu::Device,
    pub(super) queue: wgpu::Queue,
    pub(super) format: wgpu::TextureFormat,
    pub(super) renderer: Renderer,
}

pub(super) async fn create_gpu(
    instance: wgpu::Instance,
    probe: &wgpu::Surface<'_>,
    context: &Context,
    open_device: Option<super::OpenDevice>,
) -> Result<Gpu, Box<dyn Error>> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(probe),
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
    let capabilities = probe.get_capabilities(&adapter);
    let format = capabilities
        .formats
        .iter()
        .copied()
        .find(|format| format.is_srgb())
        .or_else(|| capabilities.formats.first().copied())
        .ok_or("the adapter does not support this surface")?;
    let renderer = Renderer::new(&device, format);
    context.set_renderer_info(RendererInfo {
        adapter: adapter.get_info(),
        format,
    });
    Ok(Gpu {
        instance,
        adapter,
        device,
        queue,
        format,
        renderer,
    })
}

pub(super) enum Presented {
    Done,
    Again,
}

pub(super) struct Target {
    surface: Option<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,
    prepared_size: Option<(Vec2, f32)>,
    clear_color: Option<Color32>,
    pending: Option<Repaint>,
    retained: Option<Retained>,
}

struct Retained {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: (u32, u32),
}

impl Target {
    pub(super) fn new(format: wgpu::TextureFormat) -> Self {
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
            retained: None,
        }
    }

    pub(super) fn attached(&self) -> bool {
        self.surface.is_some()
    }

    pub(super) fn physical(&self) -> Option<Vec2> {
        if self.surface.is_none() || self.config.width == 0 || self.config.height == 0 {
            return None;
        }
        Some(vec2(self.config.width as f32, self.config.height as f32))
    }

    pub(super) fn attach(
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

    pub(super) fn detach(&mut self) {
        self.surface = None;
        self.retained = None;
        self.pending = None;
    }

    pub(super) fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
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
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
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

    pub(super) fn prepare(
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
        let repaint = match output.damage() {
            Some(region) if !stale => Repaint::Region {
                region,
                background: clear_color,
            },
            _ => Repaint::Everything,
        };
        if output.changed || stale {
            let repaint = match self.pending {
                Some(pending) => pending.union(repaint),
                None => repaint,
            };
            let effective =
                gpu.renderer
                    .prepare(&gpu.device, &gpu.queue, output, physical, scale, repaint);
            self.prepared_size = Some(size);
            self.pending = Some(effective);
        }
        self.clear_color = Some(clear_color);
        self.pending.is_some()
    }

    pub(super) fn present(&mut self, gpu: &mut Gpu, background: Color32) -> Presented {
        if self.config.width == 0 || self.config.height == 0 {
            return Presented::Done;
        }
        let Some(surface) = &self.surface else {
            return Presented::Done;
        };
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                surface.configure(&gpu.device, &self.config);
                return Presented::Again;
            }
            _ => return Presented::Done,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beui encoder"),
            });
        let clear = clear_color(background);
        self.retain(&gpu.device);
        let pending = self.pending.take();
        let size = (self.config.width, self.config.height);
        let retained = self.retained.as_ref();
        let (target, load) = match retained {
            Some(retained) => (
                &retained.view,
                match pending {
                    Some(Repaint::Region { .. }) => wgpu::LoadOp::Load,
                    _ => wgpu::LoadOp::Clear(clear),
                },
            ),
            None => (&view, wgpu::LoadOp::Clear(clear)),
        };
        if pending.is_some() || retained.is_none() {
            gpu.renderer
                .render(&gpu.device, &gpu.queue, &mut encoder, target, size, load);
        }
        if let Some(retained) = retained {
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
        frame.present();
        Presented::Done
    }
}

pub(super) fn safe_rect(screen: Vec2, area: super::SafeArea, scale: f32) -> crate::geometry::Rect {
    use crate::geometry::{Rect, pos2};
    let min = pos2(area.left / scale, area.top / scale);
    let max = pos2(
        screen.x - area.right / scale,
        screen.y - area.bottom / scale,
    );
    Rect::from_min_max(min, pos2(max.x.max(min.x), max.y.max(min.y)))
}
