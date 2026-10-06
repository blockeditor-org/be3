use std::collections::HashMap;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::OffscreenCanvas;

#[derive(Clone, Debug)]
struct Display;

impl wgpu::rwh::HasDisplayHandle for Display {
    fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        Ok(wgpu::rwh::DisplayHandle::web())
    }
}

pub(crate) struct Screens {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    atlas: Option<Painted>,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
    shown: HashMap<u32, Shown>,
}

struct Shown {
    surface: u32,
    stale: bool,
    target: Target,
}

enum Target {
    Surface(Painted),
    Copied {
        canvas: OffscreenCanvas,
        context: JsValue,
    },
}

struct Painted {
    canvas: OffscreenCanvas,
    surface: wgpu::Surface<'static>,
    formats: Vec<wgpu::TextureFormat>,
    alpha: wgpu::CompositeAlphaMode,
    configured: Option<wgpu::SurfaceConfiguration>,
}

impl Screens {
    pub(crate) async fn open() -> Result<(Self, wgpu::Device, wgpu::Queue), String> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
        descriptor.display = Some(Box::new(Display));
        let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
        let canvas = OffscreenCanvas::new(1, 1).map_err(|_| "no OffscreenCanvas".to_owned())?;
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas.clone()))
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
                label: Some("plugin device"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|error| error.to_string())?;
        let atlas = (adapter.get_info().backend == wgpu::Backend::Gl)
            .then(|| Painted::new(&adapter, canvas, surface));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("plugin screen layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("plugin screen pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("screen.wgsl"));
        let screens = Self {
            instance,
            adapter,
            device: device.clone(),
            queue: queue.clone(),
            atlas,
            layout,
            pipeline_layout,
            shader,
            pipelines: HashMap::new(),
            shown: HashMap::new(),
        };
        Ok((screens, device, queue))
    }

    pub(crate) fn show(
        &mut self,
        id: u32,
        canvas: Option<OffscreenCanvas>,
        surface: u32,
    ) -> Result<(), String> {
        if let Some(canvas) = canvas {
            let target = match self.atlas {
                Some(_) => {
                    let context = canvas
                        .get_context("2d")
                        .ok()
                        .flatten()
                        .ok_or("a plugin screen's canvas has no 2d context")?;
                    Target::Copied {
                        canvas,
                        context: context.into(),
                    }
                }
                None => {
                    let surface = self
                        .instance
                        .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas.clone()))
                        .map_err(|error| error.to_string())?;
                    Target::Surface(Painted::new(&self.adapter, canvas, surface))
                }
            };
            self.shown.insert(
                id,
                Shown {
                    surface,
                    stale: true,
                    target,
                },
            );
            return Ok(());
        }
        if let Some(shown) = self.shown.get_mut(&id)
            && shown.surface != surface
        {
            shown.surface = surface;
            shown.stale = true;
        }
        Ok(())
    }

    pub(crate) fn forget(&mut self, id: u32) {
        self.shown.remove(&id);
    }

    pub(crate) fn presented(&mut self, surfaces: &[u32]) {
        for shown in self.shown.values_mut() {
            if surfaces.contains(&shown.surface) {
                shown.stale = true;
            }
        }
    }

    pub(crate) fn paint<'a>(
        &mut self,
        surface: impl Fn(u32) -> Option<(&'a wgpu::Texture, u64)>,
    ) -> Result<(), String> {
        let mut shown = std::mem::take(&mut self.shown);
        let mut result = Ok(());
        for screen in shown.values_mut().filter(|shown| shown.stale) {
            let Some((texture, _)) = surface(screen.surface) else {
                continue;
            };
            screen.stale = false;
            if let Err(error) = self.show_texture(texture, &mut screen.target) {
                result = Err(error);
            }
        }
        self.shown = shown;
        result
    }

    fn show_texture(&mut self, texture: &wgpu::Texture, target: &mut Target) -> Result<(), String> {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let size = [texture.width(), texture.height()];
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("plugin screen"),
            });
        let frame = match target {
            Target::Surface(painted) => {
                self.draw(&mut encoder, &view, texture.format(), painted, size)
            }
            Target::Copied { .. } => {
                let mut atlas = self
                    .atlas
                    .take()
                    .ok_or("a copied plugin screen has no atlas to copy from")?;
                let drawn = self.draw(&mut encoder, &view, texture.format(), &mut atlas, size);
                self.atlas = Some(atlas);
                drawn
            }
        }?;
        self.queue.submit(Some(encoder.finish()));
        if let Some(frame) = frame {
            frame.present();
        }
        if let (Target::Copied { canvas, context }, Some(atlas)) = (target, &self.atlas) {
            copy(&atlas.canvas, canvas, context, size)?;
        }
        Ok(())
    }

    fn draw(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        source: wgpu::TextureFormat,
        painted: &mut Painted,
        size: [u32; 2],
    ) -> Result<Option<wgpu::SurfaceTexture>, String> {
        if size[0] == 0 || size[1] == 0 {
            return Ok(None);
        }
        let format = painted.configure(&self.device, source, size)?;
        let frame = match painted.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            status => {
                return Err(format!(
                    "a plugin screen's canvas is unavailable: {status:?}"
                ));
            }
        };
        let target = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(format),
            ..Default::default()
        });
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("plugin screen"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            }],
        });
        let pipeline = self.pipeline(format);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("plugin screen"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
        drop(pass);
        Ok(Some(frame))
    }

    fn pipeline(&mut self, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        self.pipelines
            .entry(format)
            .or_insert_with(|| {
                self.device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("plugin screen pipeline"),
                        layout: Some(&self.pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &self.shader,
                            entry_point: Some("screen_vertex"),
                            compilation_options: Default::default(),
                            buffers: &[],
                        },
                        primitive: Default::default(),
                        depth_stencil: None,
                        multisample: Default::default(),
                        fragment: Some(wgpu::FragmentState {
                            module: &self.shader,
                            entry_point: Some("screen_fragment"),
                            compilation_options: Default::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        multiview_mask: None,
                        cache: None,
                    })
            })
            .clone()
    }
}

impl Painted {
    fn new(
        adapter: &wgpu::Adapter,
        canvas: OffscreenCanvas,
        surface: wgpu::Surface<'static>,
    ) -> Self {
        let mut capabilities = surface.get_capabilities(adapter);
        if adapter.get_info().backend == wgpu::Backend::BrowserWebGpu {
            capabilities
                .alpha_modes
                .push(wgpu::CompositeAlphaMode::PreMultiplied);
        }
        Self::with(canvas, surface, capabilities)
    }

    fn with(
        canvas: OffscreenCanvas,
        surface: wgpu::Surface<'static>,
        capabilities: wgpu::SurfaceCapabilities,
    ) -> Self {
        let alpha = match capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            true => wgpu::CompositeAlphaMode::PreMultiplied,
            false => capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
        };
        Self {
            canvas,
            surface,
            formats: capabilities.formats,
            alpha,
            configured: None,
        }
    }

    fn configure(
        &mut self,
        device: &wgpu::Device,
        source: wgpu::TextureFormat,
        [width, height]: [u32; 2],
    ) -> Result<wgpu::TextureFormat, String> {
        let srgb = source.is_srgb();
        let (format, view) =
            self.formats
                .iter()
                .find(|format| eight_bit(**format) && format.is_srgb() == srgb)
                .map(|format| (*format, *format))
                .or_else(|| {
                    self.formats.iter().find(|format| eight_bit(**format)).map(
                        |format| match srgb {
                            true => (*format, format.add_srgb_suffix()),
                            false => (*format, format.remove_srgb_suffix()),
                        },
                    )
                })
                .ok_or("this browser cannot show a plugin's canvas in an 8-bit format")?;
        let configuration = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 1,
            alpha_mode: self.alpha,
            view_formats: match view == format {
                true => Vec::new(),
                false => vec![view],
            },
        };
        if self.configured.as_ref() != Some(&configuration) {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
            self.surface.configure(device, &configuration);
            self.configured = Some(configuration);
        }
        Ok(view)
    }
}

fn eight_bit(format: wgpu::TextureFormat) -> bool {
    matches!(
        format.remove_srgb_suffix(),
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
    )
}

fn copy(
    atlas: &OffscreenCanvas,
    canvas: &OffscreenCanvas,
    context: &JsValue,
    [width, height]: [u32; 2],
) -> Result<(), String> {
    if canvas.width() != width || canvas.height() != height {
        canvas.set_width(width);
        canvas.set_height(height);
    }
    let call = |name: &str, arguments: &[JsValue]| {
        js_sys::Reflect::get(context, &name.into())
            .and_then(|method| method.dyn_into::<js_sys::Function>())
            .and_then(|method| method.apply(context, &arguments.iter().collect::<js_sys::Array>()))
            .map(|_| ())
            .map_err(|error| format!("a plugin screen could not be copied: {error:?}"))
    };
    let number = |value: u32| JsValue::from(value);
    call(
        "clearRect",
        &[number(0), number(0), number(width), number(height)],
    )?;
    if width == 0 || height == 0 {
        return Ok(());
    }
    call(
        "drawImage",
        &[
            JsValue::from(atlas.clone()),
            number(0),
            number(0),
            number(width),
            number(height),
            number(0),
            number(0),
            number(width),
            number(height),
        ],
    )
}
