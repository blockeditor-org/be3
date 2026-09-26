use block_gpu_abi as abi;

#[derive(Clone, Debug)]
struct Display;

impl wgpu::rwh::HasDisplayHandle for Display {
    fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        Ok(wgpu::rwh::DisplayHandle::web())
    }
}

pub(crate) struct Canvas {
    canvas: web_sys::OffscreenCanvas,
    surface: wgpu::Surface<'static>,
    configuration: Option<wgpu::SurfaceConfiguration>,
    formats: Vec<wgpu::TextureFormat>,
    alpha: wgpu::CompositeAlphaMode,
    copy: Option<CanvasCopy>,
    picture: Option<web_sys::ImageBitmap>,
}

struct CanvasCopy {
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

const COPY_SHADER: &str = "
@group(0) @binding(0) var drawn: texture_2d<f32>;

@vertex
fn copy_vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn copy_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(drawn, vec2<i32>(position.xy), 0);
}
";

impl CanvasCopy {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("plugin canvas copy"),
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
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("plugin canvas copy"),
            source: wgpu::ShaderSource::Wgsl(COPY_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("plugin canvas copy"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("plugin canvas copy"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("copy_vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("copy_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            format,
            layout,
            pipeline,
        }
    }
}

impl Canvas {
    pub(crate) async fn open(
        canvas: web_sys::OffscreenCanvas,
    ) -> Result<(Self, wgpu::Device, wgpu::Queue), String> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.display = Some(Box::new(Display));
        let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas.clone()))
            .map_err(|error| error.to_string())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .map_err(|error| error.to_string())?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("plugin canvas"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|error| error.to_string())?;
        let capabilities = surface.get_capabilities(&adapter);
        let alpha = if adapter.get_info().backend == wgpu::Backend::BrowserWebGpu {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            capabilities
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto)
        };
        let canvas = Self {
            canvas,
            surface,
            configuration: None,
            formats: capabilities.formats,
            alpha,
            copy: None,
            picture: None,
        };
        Ok((canvas, device, queue))
    }

    pub(crate) fn configure(
        &mut self,
        device: &wgpu::Device,
        requested: &abi::SurfaceConfiguration,
    ) -> Result<(), String> {
        let format = format(requested.format);
        if !self.formats.contains(&format) {
            return Err(format!(
                "this browser cannot show a plugin canvas as {format:?}"
            ));
        }
        let width = requested.width.max(1);
        let height = requested.height.max(1);
        self.canvas.set_width(width);
        self.canvas.set_height(height);
        let configuration = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 1,
            alpha_mode: self.alpha,
            view_formats: vec![format],
        };
        self.surface.configure(device, &configuration);
        self.configuration = Some(configuration);
        Ok(())
    }

    pub(crate) fn present(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        drawn: &wgpu::Texture,
    ) -> Result<(), String> {
        let Some(configuration) = self.configuration.clone() else {
            return Err("a plugin presented before its canvas was configured".to_owned());
        };
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(device, &configuration);
                match self.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
                    status => return Err(format!("the plugin canvas is unavailable: {status:?}")),
                }
            }
            status => return Err(format!("the plugin canvas is unavailable: {status:?}")),
        };
        let format = configuration.format;
        let copy = match self.copy.take() {
            Some(copy) if copy.format == format => copy,
            _ => CanvasCopy::new(device, format),
        };
        let source = drawn.create_view(&wgpu::TextureViewDescriptor::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("plugin canvas copy"),
            layout: &copy.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source),
            }],
        });
        let target = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("plugin canvas copy"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("plugin canvas copy"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&copy.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.copy = Some(copy);
        queue.submit([encoder.finish()]);
        frame.present();
        let picture = self
            .canvas
            .transfer_to_image_bitmap()
            .map_err(|_| "the plugin canvas could not be shown".to_owned())?;
        if let Some(previous) = self.picture.replace(picture) {
            previous.close();
        }
        Ok(())
    }

    pub(crate) fn take_picture(&mut self) -> Option<web_sys::ImageBitmap> {
        self.picture.take()
    }
}

fn format(format: abi::TextureFormat) -> wgpu::TextureFormat {
    block_gpu_host::texture_format(format)
}
