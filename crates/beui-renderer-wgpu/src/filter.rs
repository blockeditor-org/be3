//! A filter (`Group::Filter`): what it covers is drawn into a scene texture, Vello blurs that
//! into a second texture (a Gaussian blur of σ, clamped at the filter's region), and a pass of
//! ours composes either over the target, applying the contrast and the colour vision matrix
//! within the region (vello_gpu 0.4 has no colour matrix filter).

use bytemuck::{Pod, Zeroable};

use beui_core::filter::Filter;
use beui_types::Vec2;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CombineUniforms {
    region: [f32; 4],
    rows: [[f32; 4]; 3],
    params: [f32; 4],
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Prepared {
    region: [f32; 4],
    /// The blur's standard deviation, in pixels.
    sigma: f32,
    reach: f32,
    contrast: f32,
    matrix: [[f32; 3]; 3],
}

impl Prepared {
    pub(super) fn new(filter: &Filter, pixels_per_point: f32, screen: Vec2) -> Self {
        let region = filter.region;
        let sigma = (filter.blur * pixels_per_point).max(0.0);
        Self {
            region: [
                (region.left() * pixels_per_point).max(0.0),
                (region.top() * pixels_per_point).max(0.0),
                (region.right() * pixels_per_point).min(screen.x),
                (region.bottom() * pixels_per_point).min(screen.y),
            ],
            sigma,
            // A Gaussian holds all but 0.3% of its weight within 3σ, as Vello's blur reaches.
            reach: match sigma > 0.0 {
                true => (sigma * 3.0).ceil() + 1.0,
                false => 0.0,
            },
            contrast: filter.contrast.clamp(0.0, 1.0),
            matrix: filter.vision.matrix(),
        }
    }

    pub(super) fn region(&self) -> [f32; 4] {
        self.region
    }

    pub(super) fn sigma(&self) -> f32 {
        self.sigma
    }


    pub(super) fn widen(&self, damage: [f32; 4]) -> [f32; 4] {
        let touched = overlap(damage, self.region);
        if self.reach <= 0.0 || !covers(touched) {
            return damage;
        }
        let spread = overlap(grown(touched, self.reach), self.region);
        [
            damage[0].min(spread[0]),
            damage[1].min(spread[1]),
            damage[2].max(spread[2]),
            damage[3].max(spread[3]),
        ]
    }
}

fn overlap(one: [f32; 4], other: [f32; 4]) -> [f32; 4] {
    [
        one[0].max(other[0]),
        one[1].max(other[1]),
        one[2].min(other[2]),
        one[3].min(other[3]),
    ]
}

fn grown(rect: [f32; 4], amount: f32) -> [f32; 4] {
    [
        rect[0] - amount,
        rect[1] - amount,
        rect[2] + amount,
        rect[3] + amount,
    ]
}

fn covers(rect: [f32; 4]) -> bool {
    rect[0] < rect[2] && rect[1] < rect[3]
}

pub(super) struct Effects {
    format: wgpu::TextureFormat,
    sampler: wgpu::Sampler,
    layout: wgpu::BindGroupLayout,
    compose: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    /// What the filter covers, unfiltered, and blurred.
    scene: Option<(wgpu::TextureView, wgpu::TextureView)>,
    size: (u32, u32),
    bind: Option<wgpu::BindGroup>,
}

impl Effects {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("beui combine layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
                sampler_entry(3),
            ],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("combine.wgsl"));
        let compose = pipeline(
            device,
            &layout,
            &shader,
            format,
            "beui combine",
            "compose",
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        );
        Self {
            format,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("beui filter sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            layout,
            compose,
            uniforms: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("beui combine uniforms"),
                size: std::mem::size_of::<CombineUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            scene: None,
            size: (0, 0),
            bind: None,
        }
    }

    pub(super) fn ensure(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        if self.size == size && self.scene.is_some() {
            return;
        }
        self.size = size;
        self.bind = None;
        self.scene = match size.0 == 0 || size.1 == 0 {
            true => None,
            false => Some((
                self.layer(device, size, "beui filter scene"),
                self.layer(device, size, "beui filter blur"),
            )),
        };
    }

    /// The texture what the filter covers is drawn into.
    pub(super) fn scene(&self) -> Option<&wgpu::TextureView> {
        self.scene.as_ref().map(|(scene, _)| scene)
    }

    /// The texture Vello blurs the scene into.
    pub(super) fn blurred(&self) -> Option<&wgpu::TextureView> {
        self.scene.as_ref().map(|(_, blurred)| blurred)
    }

    /// Readies the pass that composes the scene, blurred when `prepared` blurs, over the
    /// target.
    pub(super) fn record(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        prepared: &Prepared,
        stored_linear: bool,
    ) {
        let (width, height) = self.size;
        let Some((scene, blurred)) = &self.scene else {
            self.bind = None;
            return;
        };
        let region = [
            prepared.region[0] / width as f32,
            prepared.region[1] / height as f32,
            prepared.region[2] / width as f32,
            prepared.region[3] / height as f32,
        ];
        let blurs = prepared.sigma > 0.0;
        queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::bytes_of(&CombineUniforms {
                region,
                rows: rows(prepared.matrix),
                params: [
                    prepared.contrast,
                    if stored_linear { 1.0 } else { 0.0 },
                    0.0,
                    0.0,
                ],
            }),
        );
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("beui combine bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(scene),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(match blurs {
                        true => blurred,
                        false => scene,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.bind = Some(bind);
    }

    pub(super) fn compose(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(bind) = &self.bind else {
            return;
        };
        pass.set_pipeline(&self.compose);
        pass.set_bind_group(0, bind, &[]);
        pass.draw(0..3, 0..1);
    }

    fn layer(&self, device: &wgpu::Device, size: (u32, u32), label: &str) -> wgpu::TextureView {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size.0.max(1),
                    height: size.1.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: crate::view_formats(self.format),
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    }
}

fn rows(matrix: [[f32; 3]; 3]) -> [[f32; 4]; 3] {
    matrix.map(|row| [row[0], row[1], row[2], 0.0])
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    label: &str,
    entry_point: &str,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("full"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
