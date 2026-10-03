use std::error::Error;

use beui_core::app::Setup;
use beui_core::color::Color32;
use beui_core::context::{FrameOutput, RendererInfo};
use beui_core::geometry::Vec2;
use beui_core::renderer;

use crate::present::{GpuSetup, surface_format};
use crate::{Renderer, Repaint, clear_color_in, renderer_info};

#[derive(Clone, Debug)]
struct Display;

impl wgpu::rwh::HasDisplayHandle for Display {
    fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        Ok(wgpu::rwh::DisplayHandle::web())
    }
}

pub struct CanvasSurface {
    canvas: web_sys::HtmlCanvasElement,
    display: Option<String>,
    info: RendererInfo,
    _instance: wgpu::Instance,
    _adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    prepared: Option<(Vec2, f32, Color32)>,
}

impl CanvasSurface {
    pub async fn new(canvas: web_sys::HtmlCanvasElement) -> Result<Self, Box<dyn Error>> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
        descriptor.display = Some(Box::new(Display));
        let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|error| error.to_string())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .map_err(|error| error.to_string())?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("beui device"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|error| error.to_string())?;
        let capabilities = surface.get_capabilities(&adapter);
        let format =
            surface_format(&capabilities.formats).ok_or("the adapter cannot show a canvas")?;
        let alpha_mode = match capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
            || adapter.get_info().backend == wgpu::Backend::BrowserWebGpu
        {
            true => wgpu::CompositeAlphaMode::PreMultiplied,
            false => capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
        };
        let info = renderer_info(&adapter.get_info(), format);
        let renderer = Renderer::new(&device, format);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: 0,
            height: 0,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: Vec::new(),
        };
        Ok(Self {
            canvas,
            display: None,
            info,
            _instance: instance,
            _adapter: adapter,
            device,
            queue,
            surface,
            config,
            renderer,
            prepared: None,
        })
    }

    pub fn gpu(&self) -> GpuSetup {
        GpuSetup {
            device: self.device.clone(),
            queue: self.queue.clone(),
            format: self.config.format,
        }
    }

    fn present_frame(&mut self, background: Color32) -> bool {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                self.prepared = None;
                return true;
            }
            _ => return false,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beui encoder"),
            });
        let clear = clear_color_in(self.config.format, background);
        self.renderer.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            (self.config.width, self.config.height),
            wgpu::LoadOp::Clear(clear),
        );
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        false
    }
}

impl renderer::Renderer for CanvasSurface {
    fn name(&self) -> &'static str {
        "wgpu"
    }

    fn info(&self) -> RendererInfo {
        self.info.clone()
    }

    fn provide(&self, setup: &mut Setup) {
        setup.provide(self.gpu());
    }

    fn set_active(&mut self, active: bool) {
        let style = self.canvas.style();
        match active {
            true => {
                if let Some(display) = self.display.take() {
                    let _ = style.set_property("display", &display);
                }
            }
            false => {
                if self.display.is_none() {
                    self.display = style.get_property_value("display").ok();
                }
                let _ = style.set_property("display", "none");
            }
        }
        self.prepared = None;
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    fn physical(&self) -> Option<Vec2> {
        (self.config.width > 0 && self.config.height > 0)
            .then(|| Vec2::new(self.config.width as f32, self.config.height as f32))
    }

    fn prepare(&mut self, output: &FrameOutput, scale: f32, background: Color32) -> bool {
        let Some(physical) = self.physical() else {
            return false;
        };
        let stale = self.prepared != Some((physical, scale, background));
        if !(output.changed || stale) {
            return false;
        }
        self.renderer.prepare(
            &self.device,
            &self.queue,
            output,
            physical,
            scale,
            Repaint::Everything,
        );
        self.prepared = Some((physical, scale, background));
        true
    }

    fn present(&mut self, background: Color32) -> bool {
        self.present_frame(background)
    }
}
