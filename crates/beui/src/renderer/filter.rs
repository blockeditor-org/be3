use bytemuck::{Pod, Zeroable};

use crate::filter::Filter;
use crate::geometry::Vec2;

const MAX_LEVELS: usize = 6;
const MIN_OFFSET: f32 = 0.5;
const MAX_OFFSET: f32 = 4.0;
const MAX_PASSES: u64 = (MAX_LEVELS * 2) as u64;
const UNIFORM_STRIDE: u64 = 256;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurUniforms {
    texel: [f32; 4],
    bounds: [f32; 4],
}

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
    levels: usize,
    offset: f32,
    strength: f32,
    reach: f32,
    contrast: f32,
    matrix: [[f32; 3]; 3],
}

impl Prepared {
    pub(super) fn new(filter: &Filter, pixels_per_point: f32, screen: Vec2) -> Self {
        let region = filter.region;
        let (levels, offset, strength) = steps(filter.blur * pixels_per_point, screen);
        Self {
            region: [
                (region.left() * pixels_per_point).max(0.0),
                (region.top() * pixels_per_point).max(0.0),
                (region.right() * pixels_per_point).min(screen.x),
                (region.bottom() * pixels_per_point).min(screen.y),
            ],
            levels,
            offset,
            strength,
            reach: reach(levels, offset),
            contrast: filter.contrast.clamp(0.0, 1.0),
            matrix: filter.vision.matrix(),
        }
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

struct Layer {
    view: wgpu::TextureView,
    bind: wgpu::BindGroup,
    size: (u32, u32),
}

pub(super) struct Effects {
    format: wgpu::TextureFormat,
    sampler: wgpu::Sampler,
    blur_layout: wgpu::BindGroupLayout,
    combine_layout: wgpu::BindGroupLayout,
    downsample: wgpu::RenderPipeline,
    upsample: wgpu::RenderPipeline,
    compose: wgpu::RenderPipeline,
    blur_uniforms: wgpu::Buffer,
    combine_uniforms: wgpu::Buffer,
    scene: Option<Layer>,
    chain: Vec<Layer>,
    size: (u32, u32),
    combine_bind: Option<wgpu::BindGroup>,
}

impl Effects {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("beui blur layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<BlurUniforms>() as u64,
                        ),
                    },
                    count: None,
                },
                texture_entry(1),
                sampler_entry(2),
            ],
        });
        let combine_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let blur_shader = device.create_shader_module(wgpu::include_wgsl!("blur.wgsl"));
        let combine_shader = device.create_shader_module(wgpu::include_wgsl!("combine.wgsl"));
        let downsample = pipeline(
            device,
            &blur_layout,
            &blur_shader,
            format,
            "beui downsample",
            "downsample",
        );
        let upsample = pipeline(
            device,
            &blur_layout,
            &blur_shader,
            format,
            "beui upsample",
            "upsample",
        );
        let compose = pipeline(
            device,
            &combine_layout,
            &combine_shader,
            format,
            "beui combine",
            "compose",
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
            blur_layout,
            combine_layout,
            downsample,
            upsample,
            compose,
            blur_uniforms: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("beui blur uniforms"),
                size: MAX_PASSES * UNIFORM_STRIDE,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            combine_uniforms: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("beui combine uniforms"),
                size: std::mem::size_of::<CombineUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            scene: None,
            chain: Vec::new(),
            size: (0, 0),
            combine_bind: None,
        }
    }

    pub(super) fn ensure(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        if self.size == size && self.scene.is_some() {
            return;
        }
        self.size = size;
        self.chain.clear();
        self.combine_bind = None;
        self.scene = match size.0 == 0 || size.1 == 0 {
            true => None,
            false => Some(self.layer(device, size, "beui filter scene")),
        };
    }

    pub(super) fn release(&mut self) {
        self.scene = None;
        self.chain.clear();
        self.combine_bind = None;
        self.size = (0, 0);
    }

    pub(super) fn scene(&self) -> Option<&wgpu::TextureView> {
        self.scene.as_ref().map(|layer| &layer.view)
    }

    pub(super) fn record(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        prepared: &Prepared,
        stored_linear: bool,
        scissor: Option<[u32; 4]>,
    ) {
        let (width, height) = self.size;
        if width == 0 || height == 0 || self.scene.is_none() {
            self.combine_bind = None;
            return;
        }
        let region = [
            prepared.region[0] / width as f32,
            prepared.region[1] / height as f32,
            prepared.region[2] / width as f32,
            prepared.region[3] / height as f32,
        ];
        let (levels, offset) = (prepared.levels, prepared.offset);
        if levels > 0 {
            self.grow(device, levels);
        }
        for (pass, (source, target)) in Self::route(levels).into_iter().enumerate() {
            let size = self.source_size(source);
            let texel = [1.0 / size.0 as f32, 1.0 / size.1 as f32, offset, 0.0];
            let bounds = [
                region[0] + texel[0],
                region[1] + texel[1],
                (region[2] - texel[0]).max(region[0] + texel[0]),
                (region[3] - texel[1]).max(region[1] + texel[1]),
            ];
            queue.write_buffer(
                &self.blur_uniforms,
                pass as u64 * UNIFORM_STRIDE,
                bytemuck::bytes_of(&BlurUniforms { texel, bounds }),
            );
            let down = target > source;
            let clip = scissor.map(|scissor| narrowed(scissor, target, self.chain[target].size));
            if clip.is_some_and(|[_, _, width, height]| width == 0 || height == 0) {
                continue;
            }
            let mut render = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("beui blur pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.chain[target].view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: match clip {
                            Some(_) => wgpu::LoadOp::Load,
                            None => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some([left, top, width, height]) = clip {
                render.set_scissor_rect(left, top, width, height);
            }
            render.set_pipeline(match down {
                true => &self.downsample,
                false => &self.upsample,
            });
            render.set_bind_group(
                0,
                self.source_bind(source),
                &[(pass as u64 * UNIFORM_STRIDE) as u32],
            );
            render.draw(0..3, 0..1);
        }
        queue.write_buffer(
            &self.combine_uniforms,
            0,
            bytemuck::bytes_of(&CombineUniforms {
                region,
                rows: rows(prepared.matrix),
                params: [
                    prepared.contrast,
                    if stored_linear { 1.0 } else { 0.0 },
                    prepared.strength,
                    0.0,
                ],
            }),
        );
        let scene = &self
            .scene
            .as_ref()
            .expect("the scene layer is present")
            .view;
        let blurred = match levels {
            0 => scene,
            _ => &self.chain[0].view,
        };
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("beui combine bind group"),
            layout: &self.combine_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.combine_uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(scene),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(blurred),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.combine_bind = Some(bind);
    }

    pub(super) fn compose(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(bind) = &self.combine_bind else {
            return;
        };
        pass.set_pipeline(&self.compose);
        pass.set_bind_group(0, bind, &[]);
        pass.draw(0..3, 0..1);
    }

    fn route(levels: usize) -> Vec<(usize, usize)> {
        let mut route = Vec::new();
        for level in 1..=levels {
            route.push((level - 1, level));
        }
        for level in (0..levels).rev() {
            route.push((level + 1, level));
        }
        route
    }

    fn source_size(&self, source: usize) -> (u32, u32) {
        match source {
            0 => self.size,
            level => self.chain[level].size,
        }
    }

    fn source_bind(&self, source: usize) -> &wgpu::BindGroup {
        match source {
            0 => {
                &self
                    .scene
                    .as_ref()
                    .expect("the scene layer is present")
                    .bind
            }
            level => &self.chain[level].bind,
        }
    }

    fn grow(&mut self, device: &wgpu::Device, levels: usize) {
        while self.chain.len() <= levels {
            let level = self.chain.len() as u32;
            let size = ((self.size.0 >> level).max(1), (self.size.1 >> level).max(1));
            let layer = self.layer(device, size, "beui blur layer");
            self.chain.push(layer);
        }
    }

    fn layer(&self, device: &wgpu::Device, size: (u32, u32), label: &str) -> Layer {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &self.blur_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.blur_uniforms,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<BlurUniforms>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        Layer {
            view,
            bind,
            size: (size.0.max(1), size.1.max(1)),
        }
    }
}

fn narrowed(scissor: [u32; 4], level: usize, size: (u32, u32)) -> [u32; 4] {
    let [left, top, width, height] = scissor;
    let shift = u32::try_from(level).unwrap_or(u32::MAX).min(u32::BITS - 1);
    let start = ((left >> shift).min(size.0), (top >> shift).min(size.1));
    let end = (
        (((left + width) >> shift) + 1).min(size.0),
        (((top + height) >> shift) + 1).min(size.1),
    );
    [
        start.0,
        start.1,
        end.0.saturating_sub(start.0),
        end.1.saturating_sub(start.1),
    ]
}

fn rows(matrix: [[f32; 3]; 3]) -> [[f32; 4]; 3] {
    matrix.map(|row| [row[0], row[1], row[2], 0.0])
}

fn steps(radius: f32, screen: Vec2) -> (usize, f32, f32) {
    if radius <= 0.0 {
        return (0, 0.0, 0.0);
    }
    let affordable = (screen.x.min(screen.y).max(1.0).log2() as usize).clamp(1, MAX_LEVELS);
    let wanted = ((radius / 2.0).max(1.0).log2().ceil() as usize).clamp(1, MAX_LEVELS);
    let levels = wanted.min(affordable);
    let asked = radius / (1u32 << levels) as f32;
    let offset = asked.clamp(MIN_OFFSET, MAX_OFFSET);
    (levels, offset, (asked / offset).min(1.0))
}

fn reach(levels: usize, offset: f32) -> f32 {
    if levels == 0 {
        return 0.0;
    }
    let span = (1u32 << levels) as f32;
    let down = (offset * 0.5 + 0.5) * (span - 1.0);
    let up = (offset + 0.5) * (span * 2.0 - 2.0);
    down + up + 1.0
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
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
