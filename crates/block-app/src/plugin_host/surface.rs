use std::{cell::RefCell, collections::HashMap};

use block_plugin_api::ScreenDamage;

use crate::plugin_host::backend::ShownFrame;
use crate::plugin_host::presenter::{RegionLayout, SurfacePresenter};

thread_local! {
    static GPU: RefCell<Option<(wgpu::Device, wgpu::Queue)>> = const { RefCell::new(None) };
}

pub(super) fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    GPU.with(|gpu| gpu.borrow().clone())
}

pub(super) fn stop() {
    let gpu = GPU.with(|gpu| gpu.borrow_mut().take());
    drop(gpu);
}

pub(crate) struct SurfaceFrame {
    pub(crate) textures: Vec<PresentedTexture>,
    pub(crate) presents: u64,
    pub(crate) damage: Option<Vec<ScreenDamage>>,
}

#[derive(Clone)]
pub(crate) struct PresentedTexture {
    pub(crate) surface: u32,
    pub(crate) texture: wgpu::Texture,
    pub(crate) generation: u64,
}

impl ShownFrame for SurfaceFrame {
    fn presents(&self) -> u64 {
        self.presents
    }

    fn damage(&self) -> Option<&[ScreenDamage]> {
        self.damage.as_deref()
    }

    fn set_damage(&mut self, damage: Option<Vec<ScreenDamage>>) {
        self.damage = damage;
    }
}

struct Target {
    generation: u64,
    bind_group: wgpu::BindGroup,
}

pub(crate) struct Presenter {
    pipeline: BlitPipeline,
    targets: HashMap<(u32, u32), [Option<Target>; 2]>,
}

pub(crate) fn presenter(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    regions: &RegionLayout,
    format: wgpu::TextureFormat,
) -> Result<Presenter, String> {
    GPU.with(|gpu| {
        *gpu.borrow_mut() = Some((device.clone(), queue.clone()));
    });
    Ok(Presenter {
        pipeline: BlitPipeline::new(device, regions, format),
        targets: HashMap::new(),
    })
}

impl SurfacePresenter for Presenter {
    type Frame = SurfaceFrame;

    fn replace(
        &mut self,
        device: &wgpu::Device,
        slot: u32,
        frame: &Self::Frame,
    ) -> Result<(), String> {
        for presented in &frame.textures {
            let [shown, other] = self.targets.entry((slot, presented.surface)).or_default();
            if shown
                .as_ref()
                .is_some_and(|target| target.generation == presented.generation)
            {
                continue;
            }
            std::mem::swap(shown, other);
            if shown
                .as_ref()
                .is_some_and(|target| target.generation == presented.generation)
            {
                continue;
            }
            let view = presented
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            *shown = Some(Target {
                generation: presented.generation,
                bind_group: self.pipeline.texture_group(device, &view),
            });
        }
        Ok(())
    }

    fn prepare(
        &mut self,
        _queue: &wgpu::Queue,
        _slot: u32,
        _frame: &Self::Frame,
    ) -> Result<(), String> {
        Ok(())
    }

    fn paint(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        slot: u32,
        surface: u32,
        regions: &wgpu::BindGroup,
        offset: u32,
    ) {
        let Some(texture) = self
            .targets
            .get(&(slot, surface))
            .and_then(|[shown, _]| shown.as_ref())
            .map(|target| &target.bind_group)
        else {
            return;
        };
        pass.set_pipeline(&self.pipeline.pipeline);
        pass.set_bind_group(0, texture, &[]);
        pass.set_bind_group(1, regions, &[offset]);
        pass.draw(0..6, 0..1);
    }

    fn retain(&mut self, slot: u32, surfaces: &[u32]) {
        self.targets
            .retain(|(held, surface), _| *held != slot || surfaces.contains(surface));
    }

    fn release(&mut self, slot: u32) {
        self.targets.retain(|(held, _), _| *held != slot);
    }
}

pub(crate) struct BlitPipeline {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) texture_layout: wgpu::BindGroupLayout,
    pub(super) sampler: wgpu::Sampler,
}

impl BlitPipeline {
    pub(super) fn new(
        device: &wgpu::Device,
        regions: &RegionLayout,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("blit.wgsl"));
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hosted plugin surface layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hosted plugin surface pipeline layout"),
            bind_group_layouts: &[Some(&texture_layout), Some(&regions.layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hosted plugin surface pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("blit_vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("blit_fragment"),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[("decode_srgb", f64::from(u8::from(target_format.is_srgb())))],
                    ..Default::default()
                },
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            texture_layout,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("hosted plugin surface sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }

    pub(super) fn texture_group(
        &self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hosted plugin surface bind group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}
