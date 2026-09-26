use std::error::Error;

use wasm_bindgen::JsCast;

use super::browser::{self, Host, Screen};
use super::{App, RunOptions, Setup};
use crate::color::Color32;
use crate::context::Context;
use crate::geometry::{Vec2, vec2};
use crate::renderer::{Renderer, RendererInfo, Repaint, clear_color};

#[derive(Clone, Debug)]
struct Display;

impl wgpu::rwh::HasDisplayHandle for Display {
    fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        Ok(wgpu::rwh::DisplayHandle::web())
    }
}

struct Runner {
    host: Host,
    canvas: web_sys::HtmlCanvasElement,
    _instance: wgpu::Instance,
    _adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    prepared: Option<(Vec2, f32, Color32)>,
}

impl Screen for Runner {
    fn host(&self) -> &Host {
        &self.host
    }

    fn frame(&mut self) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let ratio = window.device_pixel_ratio() as f32;
        let bounds = self.canvas.get_bounding_client_rect();
        let width = ((bounds.width() as f32) * ratio).round().max(1.0) as u32;
        let height = ((bounds.height() as f32) * ratio).round().max(1.0) as u32;
        if self.config.width != width || self.config.height != height {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
        let physical = vec2(width as f32, height as f32);
        let step = self.host.step(&window, ratio, physical);
        let background = self.host.app.clear_color();
        let stale = self.prepared != Some((physical, step.scale, background));
        if step.output.changed || stale {
            self.renderer.prepare(
                &self.device,
                &self.queue,
                &step.output,
                physical,
                step.scale,
                Repaint::Everything,
            );
            self.prepared = Some((physical, step.scale, background));
            self.present(background);
        }
        self.host.finish(&step);
    }
}

impl Runner {
    fn present(&mut self, background: Color32) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                self.prepared = None;
                browser::schedule();
                return;
            }
            _ => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beui encoder"),
            });
        let clear = match self.config.format.is_srgb() {
            true => clear_color(background),
            false => {
                let [red, green, blue, alpha] = background.to_array();
                wgpu::Color {
                    r: f64::from(red) / 255.0,
                    g: f64::from(green) / 255.0,
                    b: f64::from(blue) / 255.0,
                    a: f64::from(alpha) / 255.0,
                }
            }
        };
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
    }
}

pub async fn run_web(
    canvas_id: &str,
    options: RunOptions,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    let window = web_sys::window().ok_or("no browser window is available")?;
    let document = window
        .document()
        .ok_or("no browser document is available")?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or_else(|| format!("no element has the id {canvas_id}"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| format!("the element {canvas_id} is not a canvas"))?;
    document.set_title(&options.title);
    let agent = browser::text_agent(&document)?;

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
    let format = capabilities
        .formats
        .iter()
        .copied()
        .find(|format| !format.is_srgb())
        .or_else(|| capabilities.formats.first().copied())
        .ok_or("the adapter cannot show a canvas")?;
    let alpha_mode = capabilities
        .alpha_modes
        .first()
        .copied()
        .unwrap_or(wgpu::CompositeAlphaMode::Auto);
    let context = Context::new();
    context.set_renderer_info(RendererInfo {
        adapter: adapter.get_info(),
        format,
    });
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

    let mut app: Box<dyn App> = Box::new(app);
    app.setup(&Setup {
        device: device.clone(),
        queue: queue.clone(),
        format,
        waker: browser::waker(),
    });
    let host = Host::new(app, context, canvas.clone().into(), agent, options);
    browser::start(Runner {
        host,
        canvas,
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
