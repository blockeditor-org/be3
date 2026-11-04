use std::cell::RefCell;
use std::rc::Rc;

use be_lib_stable_hash::HashMap;

use beui_core::font::{GlyphId, GlyphImage};
use beui_types::{Image, ImageId};

use crate::{Atlas, Instance, Space, Uniforms, rgba};

#[derive(Clone)]
/// What every renderer on one device and surface format shares: pipelines,
/// samplers, the glyph atlases and uploaded images (kept while any renderer drew
/// them last frame). Give every window or panel on a device a `Renderer` over the
/// same `Resources`. A full atlas is replaced rather than cleared and a renderer
/// draws with what it was prepared against, so renderers sharing it may prepare,
/// render and submit in any order.
pub struct Resources(pub(crate) Rc<RefCell<Shared>>);

pub(crate) struct Shared {
    pub(crate) device: wgpu::Device,
    pub(crate) format: wgpu::TextureFormat,
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) punch_pipeline: wgpu::RenderPipeline,
    pub(crate) uniform_layout: wgpu::BindGroupLayout,
    pub(crate) space_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    samplers: [wgpu::Sampler; 2],
    atlas: Atlas,
    colors: Option<Atlas>,
    no_colors: wgpu::TextureView,
    pub(crate) glyphs: wgpu::BindGroup,
    pub(crate) pictures: HashMap<ImageId, Picture>,
    pub(crate) epoch: u64,
}

pub(crate) struct Picture {
    view: wgpu::TextureView,
    pub(crate) bind_groups: [wgpu::BindGroup; 2],
    users: usize,
}

impl Resources {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("ui.wgsl"));
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("beui uniforms layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Uniforms>() as u64),
                },
                count: None,
            }],
        });
        let space_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("beui space layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Space>() as u64),
                },
                count: None,
            }],
        });
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("beui texture layout"),
            entries: &[
                texture(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture(2),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("beui pipeline layout"),
            bind_group_layouts: &[
                Some(&uniform_layout),
                Some(&space_layout),
                Some(&texture_layout),
            ],
            immediate_size: 0,
        });
        let erase = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let punch_pipeline = pipeline(
            device,
            &layout,
            &shader,
            format,
            "beui punch pipeline",
            wgpu::BlendState {
                color: erase,
                alpha: erase,
            },
        );
        let pipeline = pipeline(
            device,
            &layout,
            &shader,
            format,
            "beui pipeline",
            wgpu::BlendState::ALPHA_BLENDING,
        );
        let smooth = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("beui smooth sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("beui sharp sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let atlas = Atlas::new(device, "beui glyph atlas", wgpu::TextureFormat::R8Unorm);
        let no_colors = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("beui no colour glyphs"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: rgba(format.is_srgb()),
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        let glyphs = textures(device, &texture_layout, &atlas.view, &smooth, &no_colors);
        Self(Rc::new(RefCell::new(Shared {
            device: device.clone(),
            format,
            pipeline,
            punch_pipeline,
            uniform_layout,
            space_layout,
            texture_layout,
            samplers: [nearest, smooth],
            atlas,
            colors: None,
            no_colors,
            glyphs,
            pictures: HashMap::default(),
            epoch: 0,
        })))
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.0.borrow().format
    }

    pub fn holds_colour_glyphs(&self) -> bool {
        self.0.borrow().colors.is_some()
    }

    pub fn pictures(&self) -> usize {
        self.0.borrow().pictures.len()
    }

    pub fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Shared {
    pub(crate) fn full(&self) -> bool {
        self.atlas.full || self.colors.as_ref().is_some_and(|colors| colors.full)
    }

    pub(crate) fn renew(&mut self) {
        self.atlas = Atlas::new(
            &self.device,
            "beui glyph atlas",
            wgpu::TextureFormat::R8Unorm,
        );
        if self.colors.is_some() {
            self.colors = Some(self.colors_atlas());
        }
        self.epoch += 1;
        self.rebind();
    }

    fn colors_atlas(&self) -> Atlas {
        Atlas::new(
            &self.device,
            "beui colour glyph atlas",
            rgba(self.format.is_srgb()),
        )
    }

    pub(crate) fn glyph(
        &mut self,
        queue: &wgpu::Queue,
        id: GlyphId,
        image: &GlyphImage,
    ) -> Option<[f32; 4]> {
        if !image.color {
            return self.atlas.insert(queue, id, image);
        }
        if self.colors.is_none() {
            self.colors = Some(self.colors_atlas());
            self.rebind();
        }
        self.colors.as_mut()?.insert(queue, id, image)
    }

    fn colors_view(&self) -> &wgpu::TextureView {
        self.colors
            .as_ref()
            .map_or(&self.no_colors, |colors| &colors.view)
    }

    fn rebind(&mut self) {
        self.glyphs = textures(
            &self.device,
            &self.texture_layout,
            &self.atlas.view,
            &self.samplers[1],
            self.colors_view(),
        );
        let bound: Vec<(ImageId, [wgpu::BindGroup; 2])> = self
            .pictures
            .iter()
            .map(|(id, picture)| (*id, self.picture_groups(&picture.view)))
            .collect();
        for (id, groups) in bound {
            if let Some(picture) = self.pictures.get_mut(&id) {
                picture.bind_groups = groups;
            }
        }
    }

    fn picture_groups(&self, view: &wgpu::TextureView) -> [wgpu::BindGroup; 2] {
        [0, 1].map(|smooth| {
            textures(
                &self.device,
                &self.texture_layout,
                view,
                &self.samplers[smooth],
                self.colors_view(),
            )
        })
    }

    pub(crate) fn upload(&mut self, queue: &wgpu::Queue, image: &Image) {
        if self.pictures.contains_key(&image.id()) {
            return;
        }
        let levels = mip_levels(image.width(), image.height());
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("beui image"),
            size: wgpu::Extent3d {
                width: image.width(),
                height: image.height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: rgba(self.format.is_srgb()),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut level = (image.width(), image.height(), None::<Vec<u8>>);
        for mip_level in 0..levels {
            let (width, height, ref shrunk) = level;
            let pixels = shrunk.as_deref().unwrap_or(image.pixels());
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            if mip_level + 1 < levels {
                level = halved(pixels, width, height);
            }
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_groups = self.picture_groups(&view);
        self.pictures.insert(
            image.id(),
            Picture {
                view,
                bind_groups,
                users: 0,
            },
        );
    }

    pub(crate) fn hold(&mut self, id: ImageId) {
        if let Some(picture) = self.pictures.get_mut(&id) {
            picture.users += 1;
        }
    }

    pub(crate) fn release(&mut self, id: ImageId) {
        let Some(picture) = self.pictures.get_mut(&id) else {
            return;
        };
        picture.users = picture.users.saturating_sub(1);
        if picture.users == 0 {
            self.pictures.remove(&id);
        }
    }
}

fn mip_levels(width: u32, height: u32) -> u32 {
    u32::BITS - width.max(height).max(1).leading_zeros()
}

fn halved(pixels: &[u8], width: u32, height: u32) -> (u32, u32, Option<Vec<u8>>) {
    let (to_width, to_height) = ((width / 2).max(1), (height / 2).max(1));
    let mut out = Vec::with_capacity(to_width as usize * to_height as usize * 4);
    let texel = |x: u32, y: u32| {
        let at = (y.min(height - 1) as usize * width as usize + x.min(width - 1) as usize) * 4;
        &pixels[at..at + 4]
    };
    for y in 0..to_height {
        for x in 0..to_width {
            let mut sums = [0u32; 4];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let texel = texel(x * 2 + dx, y * 2 + dy);
                let alpha = u32::from(texel[3]);
                for channel in 0..3 {
                    sums[channel] += u32::from(texel[channel]) * alpha;
                }
                sums[3] += alpha;
            }
            let colour = |sum: u32| match sums[3] {
                0 => 0,
                alpha => ((sum + alpha / 2) / alpha) as u8,
            };
            out.extend([
                colour(sums[0]),
                colour(sums[1]),
                colour(sums[2]),
                ((sums[3] + 2) / 4) as u8,
            ]);
        }
    }
    (to_width, to_height, Some(out))
}

fn textures(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    colors: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("beui textures"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(colors),
            },
        ],
    })
}

fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    label: &str,
    blend: wgpu::BlendState,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[Instance::layout()],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fragment"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
