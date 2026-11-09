//! Groups painted at an opacity: what a group holds is drawn onto a transparent layer
//! at least the size of the target, and the layer is then painted over the target at
//! the group's opacity, so shapes inside it do not show through each other. Only the
//! pixels within the group's bounds and the damage are cleared, drawn and painted. A
//! group inside a group draws onto the layer one deeper.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Opacity {
    value: [f32; 4],
}

struct Layer {
    view: wgpu::TextureView,
    bind: wgpu::BindGroup,
}

/// Layers grow in steps of this many pixels, so resizing a window does not make a
/// new layer every frame.
const GROWTH: u32 = 256;

pub(super) struct Groups {
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    compose: wgpu::RenderPipeline,
    clear: wgpu::RenderPipeline,
    stride: u64,
    uniforms: wgpu::Buffer,
    capacity: usize,
    layers: Vec<Layer>,
    size: (u32, u32),
}

impl Groups {
    pub(super) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("beui group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<Opacity>() as u64
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("group.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("beui group compose"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let compose = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("beui group compose"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("full"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("compose"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let empty = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("beui group clear"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let clear = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("beui group clear"),
            layout: Some(&empty),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("full"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("clear"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let alignment = u64::from(device.limits().min_uniform_buffer_offset_alignment);
        let stride = (std::mem::size_of::<Opacity>() as u64).div_ceil(alignment) * alignment;
        let capacity = 4;
        Self {
            format,
            uniforms: uniforms(device, stride, capacity),
            layout,
            compose,
            clear,
            stride,
            capacity,
            layers: Vec::new(),
            size: (0, 0),
        }
    }

    /// Makes room for groups at these opacities, nested `depth` deep, over a target of
    /// `size`.
    pub(super) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        opacities: &[f32],
        depth: usize,
        size: (u32, u32),
    ) {
        if opacities.len() > self.capacity {
            self.capacity = opacities.len().next_power_of_two();
            self.uniforms = uniforms(device, self.stride, self.capacity);
            for layer in &mut self.layers {
                layer.bind = bind(device, &self.layout, &self.uniforms, &layer.view);
            }
        }
        if size.0 > self.size.0 || size.1 > self.size.1 {
            let most = device.limits().max_texture_dimension_2d;
            let grown = |wanted: u32, held: u32| {
                wanted
                    .max(held)
                    .next_multiple_of(GROWTH)
                    .min(most.max(wanted))
            };
            self.size = (grown(size.0, self.size.0), grown(size.1, self.size.1));
            self.layers.clear();
        }
        while self.layers.len() < depth {
            self.layers.push(self.layer(device));
        }
        let stride = self.stride as usize;
        let mut bytes = vec![0u8; stride * opacities.len()];
        for (index, opacity) in opacities.iter().enumerate() {
            let value = Opacity {
                value: [*opacity, 0.0, 0.0, 0.0],
            };
            bytes[index * stride..][..std::mem::size_of::<Opacity>()]
                .copy_from_slice(bytemuck::bytes_of(&value));
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.uniforms, 0, &bytes);
        }
    }

    pub(super) fn layer_view(&self, depth: usize) -> Option<&wgpu::TextureView> {
        self.layers.get(depth).map(|layer| &layer.view)
    }

    /// Makes the pass's scissored pixels transparent.
    pub(super) fn clear(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.clear);
        pass.draw(0..3, 0..1);
    }

    /// Paints the layer at `depth` into the pass at the opacity of group `index`.
    pub(super) fn compose(&self, pass: &mut wgpu::RenderPass<'_>, depth: usize, index: usize) {
        let Some(layer) = self.layers.get(depth) else {
            return;
        };
        pass.set_pipeline(&self.compose);
        pass.set_bind_group(0, &layer.bind, &[(index as u64 * self.stride) as u32]);
        pass.draw(0..3, 0..1);
    }

    fn layer(&self, device: &wgpu::Device) -> Layer {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("beui group layer"),
            size: wgpu::Extent3d {
                width: self.size.0.max(1),
                height: self.size.1.max(1),
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
        let bind = bind(device, &self.layout, &self.uniforms, &view);
        Layer { view, bind }
    }
}

fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniforms: &wgpu::Buffer,
    view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("beui group layer"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: uniforms,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<Opacity>() as u64),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(view),
            },
        ],
    })
}

fn uniforms(device: &wgpu::Device, stride: u64, capacity: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("beui group opacities"),
        size: stride * capacity as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
