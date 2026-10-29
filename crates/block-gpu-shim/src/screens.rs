use std::collections::HashMap;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{ImageBitmap, OffscreenCanvas};

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
    layers: Layers,
    shown: HashMap<u32, Shown>,
    overlay: Option<Overlay>,
    exports: HashMap<u32, Option<Target>>,
    exported: Vec<(u32, ImageBitmap)>,
}

struct Shown {
    surface: u32,
    stale: bool,
    target: Target,
}

struct Layers {
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
}

struct Overlay {
    target: Target,
    size: [u32; 2],
    pieces: Vec<Piece>,
    stale: bool,
}

struct Piece {
    surface: u32,
    target: [f32; 4],
    source: [f32; 4],
}

struct Draw {
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    within: Option<[u32; 4]>,
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
        let layers = Layers::new(&device);
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
            layers,
            shown: HashMap::new(),
            overlay: None,
            exports: HashMap::new(),
            exported: Vec::new(),
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
            let target = self.target(canvas)?;
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

    pub(crate) fn layers(
        &mut self,
        canvas: Option<OffscreenCanvas>,
        size: [u32; 2],
        pieces: &[f32],
    ) -> Result<(), String> {
        if let Some(canvas) = canvas {
            self.overlay = Some(Overlay {
                target: self.target(canvas)?,
                size,
                pieces: Vec::new(),
                stale: true,
            });
        }
        let Some(overlay) = &mut self.overlay else {
            return Ok(());
        };
        overlay.size = size;
        overlay.pieces = pieces
            .chunks_exact(9)
            .map(|piece| Piece {
                surface: piece[0] as u32,
                target: [piece[1], piece[2], piece[3], piece[4]],
                source: [piece[5], piece[6], piece[7], piece[8]],
            })
            .collect();
        overlay.stale = true;
        Ok(())
    }

    pub(crate) fn export(&mut self, surfaces: Vec<u32>) {
        self.exports.retain(|surface, _| surfaces.contains(surface));
        for surface in surfaces {
            self.exports.entry(surface).or_insert(None);
        }
    }

    pub(crate) fn take_exported(&mut self) -> Vec<(u32, ImageBitmap)> {
        std::mem::take(&mut self.exported)
    }

    pub(crate) fn paint_exports<'a>(
        &mut self,
        presented: &[u32],
        surface: impl Fn(u32) -> Option<(&'a wgpu::Texture, u64)>,
    ) -> Result<(), String> {
        let mut exports = std::mem::take(&mut self.exports);
        let mut result = Ok(());
        for (exported, target) in &mut exports {
            if !presented.contains(exported) {
                continue;
            }
            let Some((texture, _)) = surface(*exported) else {
                continue;
            };
            let painted = self.export_texture(texture, target);
            match painted {
                Ok(bitmap) => self.exported.push((*exported, bitmap)),
                Err(error) => result = Err(error),
            }
        }
        self.exports = exports;
        result
    }

    fn export_texture(
        &mut self,
        texture: &wgpu::Texture,
        target: &mut Option<Target>,
    ) -> Result<ImageBitmap, String> {
        if target.is_none() {
            let canvas = OffscreenCanvas::new(texture.width().max(1), texture.height().max(1))
                .map_err(|_| "a preview's canvas could not be made".to_owned())?;
            *target = Some(self.target(canvas)?);
        }
        let target = target.as_mut().expect("the target was just made");
        self.show_texture(texture, target)?;
        let canvas = match target {
            Target::Surface(painted) => &painted.canvas,
            Target::Copied { canvas, .. } => canvas,
        };
        canvas
            .transfer_to_image_bitmap()
            .map_err(|error| format!("a preview could not be taken from its canvas: {error:?}"))
    }

    pub(crate) fn import(&self, bitmap: &ImageBitmap) -> wgpu::Texture {
        let size = wgpu::Extent3d {
            width: bitmap.width().max(1),
            height: bitmap.height().max(1),
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("imported preview"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.queue.copy_external_image_to_texture(
            &wgpu::CopyExternalImageSourceInfo {
                source: wgpu::ExternalImageSource::ImageBitmap(bitmap.clone()),
                origin: wgpu::Origin2d::ZERO,
                flip_y: false,
            },
            wgpu::CopyExternalImageDestInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
                color_space: wgpu::PredefinedColorSpace::Srgb,
                premultiplied_alpha: true,
            },
            size,
        );
        texture
    }

    fn target(&self, canvas: OffscreenCanvas) -> Result<Target, String> {
        Ok(match self.atlas {
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
        })
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
        if let Some(overlay) = &mut self.overlay
            && overlay
                .pieces
                .iter()
                .any(|piece| surfaces.contains(&piece.surface))
        {
            overlay.stale = true;
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
        if let Some(mut overlay) = self.overlay.take() {
            if overlay.stale {
                overlay.stale = false;
                if let Err(error) = self.paint_overlay(&mut overlay, &surface) {
                    result = Err(error);
                }
            }
            self.overlay = Some(overlay);
        }
        result
    }

    fn paint_overlay<'a>(
        &mut self,
        overlay: &mut Overlay,
        surface: &impl Fn(u32) -> Option<(&'a wgpu::Texture, u64)>,
    ) -> Result<(), String> {
        let shown: Vec<(&Piece, &wgpu::Texture)> = overlay
            .pieces
            .iter()
            .filter_map(|piece| Some((piece, surface(piece.surface)?.0)))
            .collect();
        let source = shown
            .first()
            .map_or(wgpu::TextureFormat::Rgba8Unorm, |(_, texture)| {
                texture.format()
            });
        let [width, height] = overlay.size;
        self.render(
            &mut overlay.target,
            source,
            overlay.size,
            |screens, format| {
                let pipeline = screens.layers.pipeline(&screens.device, format);
                shown
                    .iter()
                    .filter_map(|(piece, texture)| {
                        let [left, top, right, bottom] = piece.target;
                        let x = left.max(0.0).floor() as u32;
                        let y = top.max(0.0).floor() as u32;
                        let right = (right.ceil().max(0.0) as u32).min(width);
                        let bottom = (bottom.ceil().max(0.0) as u32).min(height);
                        (right > x && bottom > y).then(|| Draw {
                            pipeline: pipeline.clone(),
                            group: screens.layers.group(&screens.device, texture, piece),
                            within: Some([x, y, right - x, bottom - y]),
                        })
                    })
                    .collect()
            },
        )
    }

    fn show_texture(&mut self, texture: &wgpu::Texture, target: &mut Target) -> Result<(), String> {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let size = [texture.width(), texture.height()];
        self.render(target, texture.format(), size, |screens, format| {
            let group = screens
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("plugin screen"),
                    layout: &screens.layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    }],
                });
            vec![Draw {
                pipeline: screens.pipeline(format),
                group,
                within: None,
            }]
        })
    }

    fn render(
        &mut self,
        target: &mut Target,
        source: wgpu::TextureFormat,
        size: [u32; 2],
        draws: impl FnOnce(&mut Self, wgpu::TextureFormat) -> Vec<Draw>,
    ) -> Result<(), String> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("plugin screen"),
            });
        let frame = match target {
            Target::Surface(painted) => self.draw(&mut encoder, source, painted, size, draws),
            Target::Copied { .. } => {
                let mut atlas = self
                    .atlas
                    .take()
                    .ok_or("a copied plugin screen has no atlas to copy from")?;
                let drawn = self.draw(&mut encoder, source, &mut atlas, size, draws);
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
        source: wgpu::TextureFormat,
        painted: &mut Painted,
        size: [u32; 2],
        draws: impl FnOnce(&mut Self, wgpu::TextureFormat) -> Vec<Draw>,
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
        let draws = draws(self, format);
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
        for draw in &draws {
            if let Some([x, y, width, height]) = draw.within {
                pass.set_viewport(x as f32, y as f32, width as f32, height as f32, 0.0, 1.0);
                pass.set_scissor_rect(x, y, width, height);
            }
            pass.set_pipeline(&draw.pipeline);
            pass.set_bind_group(0, &draw.group, &[]);
            pass.draw(0..3, 0..1);
        }
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

impl Layers {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("plugin layer layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("plugin layer pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        Self {
            layout,
            pipeline_layout,
            shader: device.create_shader_module(wgpu::include_wgsl!("layer.wgsl")),
            pipelines: HashMap::new(),
        }
    }

    fn group(
        &self,
        device: &wgpu::Device,
        texture: &wgpu::Texture,
        piece: &Piece,
    ) -> wgpu::BindGroup {
        let values: Vec<u8> = piece
            .target
            .iter()
            .chain(&piece.source)
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("plugin layer piece"),
            size: values.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: true,
        });
        buffer
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(&values);
        buffer.unmap();
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("plugin layer piece"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffer.as_entire_binding(),
                },
            ],
        })
    }

    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        self.pipelines
            .entry(format)
            .or_insert_with(|| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("plugin layer pipeline"),
                    layout: Some(&self.pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &self.shader,
                        entry_point: Some("layer_vertex"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &self.shader,
                        entry_point: Some("layer_fragment"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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
