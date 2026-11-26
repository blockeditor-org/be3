//! How a canvas's configuration asks its pixels to be shown (alpha mode, color space, tone
//! mapping), turned into what every compositor of the host shows: premultiplied, sRGB-encoded
//! values in [0, 1]. A canvas already so is shown as it is; any other is drawn at each present
//! into textures the host keeps beside it, which its compositors show instead, so none of them
//! needs to know.

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

/// The pipelines that show canvases, one per format and look, made when first needed.
#[derive(Default)]
pub(crate) struct Looks {
    module: Option<wgpu::ShaderModule>,
    pipelines: HashMap<(wgpu::TextureFormat, Look), wgpu::RenderPipeline>,
}

impl Looks {
    /// Draws `canvas` as `look` asks into `shown`, a texture of its size and format.
    pub(crate) fn show(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        look: Look,
        canvas: &wgpu::Texture,
        shown: &wgpu::Texture,
    ) {
        let format = canvas.format();
        let module = self.module.get_or_insert_with(|| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("canvas look"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            })
        });
        let pipeline = self.pipelines.entry((format, look)).or_insert_with(|| {
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
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("canvas look"),
                layout: None,
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
                    targets: &[Some(format.into())],
                }),
                multiview_mask: None,
                cache: None,
            })
        });
        let source = canvas.create_view(&wgpu::TextureViewDescriptor::default());
        let target = shown.create_view(&wgpu::TextureViewDescriptor::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("canvas look"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source),
            }],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("canvas look"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("canvas look"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}
