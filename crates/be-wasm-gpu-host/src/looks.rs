//! How a canvas's configuration asks its pixels to be shown (alpha mode, color space, tone
//! mapping), turned into what every compositor of the host shows: premultiplied, sRGB-encoded
//! values in [0, 1]. A canvas already so is shown as it is; any other is drawn, at each present,
//! into rgba8unorm textures the host keeps beside it, which its compositors show instead, so
//! none of them needs to know. Every present's drawing waits for the host to take what was
//! presented (`Gpu::take_presented`), and goes in one submit.

use crate::tables::HashMap;

/// What a canvas's configuration asks of its pixels beyond what the compositors assume.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct Look {
    /// `alpha-mode` opaque, WebGPU's default: the canvas's alpha is ignored.
    pub(crate) opaque: bool,
    /// `color-space` display-p3: the values are display-p3's, which every display the host
    /// draws on (all sRGB) is shown clipped to sRGB, as browsers do.
    pub(crate) display_p3: bool,
    /// A float canvas's values past [0, 1] are clamped: no display the host draws on shows
    /// extended range, so `tone-mapping` extended is shown as standard, as browsers do.
    pub(crate) clamped: bool,
}

impl Look {
    /// Whether the canvas is shown as it is.
    pub(crate) fn plain(self) -> bool {
        self == Self::default()
    }
}

const SHADER: &str = "
override OPAQUE: bool = false;
override DISPLAY_P3: bool = false;
override CLAMPED: bool = false;

@group(0) @binding(0) var canvas: texture_2d<f32>;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}

fn decoded(encoded: vec3<f32>) -> vec3<f32> {
    return select(pow((encoded + 0.055) / 1.055, vec3<f32>(2.4)), encoded / 12.92, encoded <= vec3<f32>(0.04045));
}

fn encoded(linear: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(linear, vec3<f32>(1.0 / 2.4)) - 0.055, linear * 12.92, linear <= vec3<f32>(0.0031308));
}

// Linear display-p3 to linear sRGB, both with D65 white.
const P3_TO_SRGB = mat3x3<f32>(
    vec3<f32>(1.2249401, -0.0420569, -0.0196376),
    vec3<f32>(-0.2249404, 1.0420571, -0.0786361),
    vec3<f32>(0.0, 0.0, 1.0982735),
);

@fragment
fn fragment(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    var color = textureLoad(canvas, vec2<i32>(at.xy), 0);
    if CLAMPED {
        color = clamp(color, vec4<f32>(0.0), vec4<f32>(1.0));
    }
    if OPAQUE {
        color.a = 1.0;
    }
    if DISPLAY_P3 {
        let alpha = color.a;
        var rgb = select(color.rgb / alpha, vec3<f32>(0.0), alpha == 0.0);
        rgb = encoded(clamp(P3_TO_SRGB * decoded(rgb), vec3<f32>(0.0), vec3<f32>(1.0)));
        color = vec4<f32>(rgb * alpha, alpha);
    }
    return color;
}
";

/// What one of a canvas's textures is shown as: an rgba8unorm texture of its size, which the
/// look's pass reads the canvas texture into, with the views and bind group the pass uses,
/// made once.
pub(crate) struct Shown {
    pub(crate) texture: (wgpu::Texture, u64),
    target: wgpu::TextureView,
    group: wgpu::BindGroup,
}

/// The pass that shows canvases: its layout, made when first needed, and a pipeline per look.
#[derive(Default)]
pub(crate) struct Looks {
    made: Option<(wgpu::ShaderModule, wgpu::BindGroupLayout, wgpu::PipelineLayout)>,
    pipelines: HashMap<Look, wgpu::RenderPipeline>,
}

impl Looks {
    fn made(&mut self, device: &wgpu::Device) -> &(wgpu::ShaderModule, wgpu::BindGroupLayout, wgpu::PipelineLayout) {
        self.made.get_or_insert_with(|| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("canvas look"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
            let group = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("canvas look"),
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
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("canvas look"),
                bind_group_layouts: &[Some(&group)],
                immediate_size: 0,
            });
            (module, group, layout)
        })
    }

    /// The texture `canvas` is shown as.
    pub(crate) fn shown(&mut self, device: &wgpu::Device, canvas: &wgpu::Texture, generation: u64) -> Shown {
        let (_, layout, _) = self.made(device);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("app canvas, shown"),
            size: canvas.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: crate::SURFACE_USAGE,
            view_formats: &[],
        });
        let source = canvas.create_view(&wgpu::TextureViewDescriptor::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("canvas look"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source),
            }],
        });
        Shown {
            target: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            texture: (texture, generation),
            group,
        }
    }

    /// Records drawing a canvas into what shows it, as `look` asks.
    pub(crate) fn show(&mut self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, look: Look, shown: &Shown) {
        if !self.pipelines.contains_key(&look) {
            let (module, _, layout) = self.made(device);
            let flag = |on: bool| f64::from(u8::from(on));
            let constants = [
                ("OPAQUE", flag(look.opaque)),
                ("DISPLAY_P3", flag(look.display_p3)),
                ("CLAMPED", flag(look.clamped)),
            ];
            let options = wgpu::PipelineCompilationOptions {
                constants: &constants,
                ..Default::default()
            };
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("canvas look"),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("vertex"),
                    compilation_options: options.clone(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some("fragment"),
                    compilation_options: options,
                    targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
                }),
                multiview_mask: None,
                cache: None,
            });
            self.pipelines.insert(look, pipeline);
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("canvas look"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &shown.target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipelines[&look]);
        pass.set_bind_group(0, &shown.group, &[]);
        pass.draw(0..3, 0..1);
    }
}
