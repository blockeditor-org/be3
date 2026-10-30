use block_gpu_abi::{device as abi, embed::device::Host};

mod compute;

use crate::bundles::{BundleEncoder, Command};
use crate::queries::{Coarsening, QuerySet};
use crate::{Gpu, Outcome, SURFACE_USAGE, convert, slice, texel_copy_buffer, texel_copy_texture};

impl Gpu {
    fn try_create_bind_group(
        &mut self,
        descriptor: &abi::BindGroupDescriptor,
    ) -> Outcome<wgpu::BindGroup> {
        let layout = self
            .group_layouts
            .get(descriptor.layout.0, "bind group layout")?
            .clone();
        let mut buffers = Vec::new();
        let mut samplers = Vec::new();
        let mut views = Vec::new();
        for entry in &descriptor.entries {
            match entry.resource {
                abi::BindingResource::Buffer(binding) => {
                    buffers.push(self.buffers.get(binding.buffer.0, "buffer")?.clone());
                }
                abi::BindingResource::Sampler(sampler) => {
                    samplers.push(self.samplers.get(sampler.0, "sampler")?.clone());
                }
                abi::BindingResource::TextureView(view) => {
                    views.push(self.views.get(view.0, "texture view")?.clone());
                }
            }
        }
        let (mut buffers, mut samplers, mut views) =
            (buffers.iter(), samplers.iter(), views.iter());
        let mut entries = Vec::with_capacity(descriptor.entries.len());
        for entry in &descriptor.entries {
            let resource = match entry.resource {
                abi::BindingResource::Buffer(binding) => {
                    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: buffers.next().ok_or("a bind group lost a buffer")?,
                        offset: binding.offset,
                        size: convert::size(binding.size),
                    })
                }
                abi::BindingResource::Sampler(_) => wgpu::BindingResource::Sampler(
                    samplers.next().ok_or("a bind group lost a sampler")?,
                ),
                abi::BindingResource::TextureView(_) => wgpu::BindingResource::TextureView(
                    views.next().ok_or("a bind group lost a texture view")?,
                ),
            };
            entries.push(wgpu::BindGroupEntry {
                binding: entry.binding,
                resource,
            });
        }
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: convert::label(&descriptor.label),
            layout: &layout,
            entries: &entries,
        }))
    }

    fn try_create_render_pipeline(
        &mut self,
        descriptor: &abi::RenderPipelineDescriptor,
    ) -> Outcome<wgpu::RenderPipeline> {
        let layout = descriptor
            .layout
            .map(|handle| {
                self.pipeline_layouts
                    .get(handle.0, "pipeline layout")
                    .cloned()
            })
            .transpose()?;
        let vertex_module = self
            .modules
            .get(descriptor.vertex.module.0, "shader module")?
            .clone();
        let fragment_module = descriptor
            .fragment
            .as_ref()
            .map(|fragment| {
                self.modules
                    .get(fragment.module.0, "shader module")
                    .cloned()
            })
            .transpose()?;
        let attributes: Vec<Vec<wgpu::VertexAttribute>> = descriptor
            .vertex
            .buffers
            .iter()
            .map(|buffer| {
                buffer
                    .attributes
                    .iter()
                    .map(|attribute| wgpu::VertexAttribute {
                        format: convert::vertex_format(attribute.format),
                        offset: attribute.offset,
                        shader_location: attribute.shader_location,
                    })
                    .collect()
            })
            .collect();
        let buffers: Vec<_> = descriptor
            .vertex
            .buffers
            .iter()
            .zip(&attributes)
            .map(|(buffer, attributes)| wgpu::VertexBufferLayout {
                array_stride: buffer.array_stride,
                step_mode: convert::step_mode(buffer.step_mode),
                attributes,
            })
            .collect();
        let targets: Vec<_> = descriptor
            .fragment
            .as_ref()
            .map(|fragment| {
                fragment
                    .targets
                    .iter()
                    .map(|target| {
                        target.map(|target| wgpu::ColorTargetState {
                            format: convert::texture_format(target.format),
                            blend: target.blend.map(|blend| wgpu::BlendState {
                                color: convert::blend_component(blend.color),
                                alpha: convert::blend_component(blend.alpha),
                            }),
                            write_mask: wgpu::ColorWrites::from_bits_truncate(target.write_mask),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let primitive = descriptor.primitive;
        Ok(self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: convert::label(&descriptor.label),
                layout: layout.as_ref(),
                vertex: wgpu::VertexState {
                    module: &vertex_module,
                    entry_point: descriptor.vertex.entry_point.as_deref(),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: convert::topology(primitive.topology),
                    strip_index_format: primitive.strip_index_format.map(convert::index_format),
                    front_face: convert::front_face(primitive.front_face),
                    cull_mode: primitive.cull_mode.map(convert::face),
                    unclipped_depth: primitive.unclipped_depth,
                    polygon_mode: convert::polygon_mode(primitive.polygon_mode),
                    conservative: primitive.conservative,
                },
                depth_stencil: descriptor
                    .depth_stencil
                    .map(|state| wgpu::DepthStencilState {
                        format: convert::texture_format(state.format),
                        depth_write_enabled: Some(state.depth_write_enabled),
                        depth_compare: state.depth_compare.map(convert::compare_function),
                        stencil: wgpu::StencilState {
                            front: convert::stencil_face(state.stencil.front),
                            back: convert::stencil_face(state.stencil.back),
                            read_mask: state.stencil.read_mask,
                            write_mask: state.stencil.write_mask,
                        },
                        bias: wgpu::DepthBiasState {
                            constant: state.bias.constant,
                            slope_scale: state.bias.slope_scale,
                            clamp: state.bias.clamp,
                        },
                    }),
                multisample: wgpu::MultisampleState {
                    count: descriptor.multisample.count,
                    mask: descriptor.multisample.mask,
                    alpha_to_coverage_enabled: descriptor.multisample.alpha_to_coverage_enabled,
                },
                fragment: fragment_module.as_ref().map(|module| wgpu::FragmentState {
                    module,
                    entry_point: descriptor
                        .fragment
                        .as_ref()
                        .and_then(|fragment| fragment.entry_point.as_deref()),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            }))
    }

    fn try_begin_render_pass(
        &mut self,
        encoder: abi::CommandEncoder,
        descriptor: &abi::RenderPassDescriptor,
    ) -> Outcome<wgpu::RenderPass<'static>> {
        let mut colors = Vec::new();
        for attachment in &descriptor.color_attachments {
            let Some(attachment) = attachment else {
                colors.push(None);
                continue;
            };
            let view = self.views.get(attachment.view.0, "texture view")?.clone();
            let resolve = attachment
                .resolve_target
                .map(|handle| self.views.get(handle.0, "texture view").cloned())
                .transpose()?;
            colors.push(Some((*attachment, view, resolve)));
        }
        let depth = descriptor
            .depth_stencil_attachment
            .map(|attachment| {
                self.views
                    .get(attachment.view.0, "texture view")
                    .cloned()
                    .map(|view| (attachment, view))
            })
            .transpose()?;
        let occlusion = descriptor
            .occlusion_query_set
            .map(|set| {
                self.query_sets
                    .get(set.0, "query set")
                    .map(|set| set.set.clone())
            })
            .transpose()?;
        let timestamps = descriptor
            .timestamp_writes
            .map(|writes| {
                self.query_sets
                    .get(writes.query_set.0, "query set")
                    .map(|set| (set.set.clone(), writes))
            })
            .transpose()?;
        let encoder = self.encoders.get_mut(encoder.0, "command encoder")?;
        let color_attachments: Vec<_> = colors
            .iter()
            .map(|attachment| {
                attachment.as_ref().map(|(attachment, view, resolve)| {
                    wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: attachment.depth_slice,
                        resolve_target: resolve.as_ref(),
                        ops: wgpu::Operations {
                            load: match attachment.load {
                                abi::ColorLoadOp::Clear(value) => {
                                    wgpu::LoadOp::Clear(convert::color(value))
                                }
                                abi::ColorLoadOp::Load => wgpu::LoadOp::Load,
                            },
                            store: convert::store_op(attachment.store),
                        },
                    }
                })
            })
            .collect();
        let depth_stencil_attachment =
            depth.as_ref().map(
                |(attachment, view)| wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: attachment.depth_load.map(|load| wgpu::Operations {
                        load: match load {
                            abi::DepthLoadOp::Clear(value) => wgpu::LoadOp::Clear(value),
                            abi::DepthLoadOp::Load => wgpu::LoadOp::Load,
                        },
                        store: convert::store_op(attachment.depth_store),
                    }),
                    stencil_ops: attachment.stencil_load.map(|load| wgpu::Operations {
                        load: match load {
                            abi::StencilLoadOp::Clear(value) => wgpu::LoadOp::Clear(value),
                            abi::StencilLoadOp::Load => wgpu::LoadOp::Load,
                        },
                        store: convert::store_op(attachment.stencil_store),
                    }),
                },
            );
        Ok(encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: convert::label(&descriptor.label),
                color_attachments: &color_attachments,
                depth_stencil_attachment,
                timestamp_writes: timestamps.as_ref().map(|(set, writes)| {
                    wgpu::RenderPassTimestampWrites {
                        query_set: set,
                        beginning_of_pass_write_index: writes.beginning_of_pass_write_index,
                        end_of_pass_write_index: writes.end_of_pass_write_index,
                    }
                }),
                occlusion_query_set: occlusion.as_ref(),
                multiview_mask: None,
            })
            .forget_lifetime())
    }

    fn encode(
        &mut self,
        encoder: abi::CommandEncoder,
        act: impl FnOnce(&mut Self) -> Outcome<Box<dyn FnOnce(&mut wgpu::CommandEncoder)>>,
    ) {
        self.guard(|gpu| {
            let record = act(gpu)?;
            record(gpu.encoders.get_mut(encoder.0, "command encoder")?);
            Ok(())
        });
    }

    fn mapped_range(
        &self,
        buffer: abi::Buffer,
        offset: u64,
        size: u64,
        writing: bool,
    ) -> Outcome<(wgpu::Buffer, std::ops::Range<u64>)> {
        let mapped = self.buffers.get(buffer.0, "buffer")?.clone();
        let range = offset
            .checked_add(size)
            .map(|end| offset..end)
            .ok_or_else(|| "a mapped range ran past the end of its buffer".to_owned())?;
        self.maps.mapped(buffer.0, &range, writing)?;
        Ok((mapped, range))
    }

    fn pass(&mut self, pass: abi::RenderPass, act: impl FnOnce(&mut wgpu::RenderPass<'static>)) {
        let result = self.passes.get_mut(pass.0, "render pass").map(act);
        self.fail(result, ());
    }
}

impl Host for Gpu {
    fn features(&mut self) -> Vec<abi::FeatureName> {
        convert::features(self.device.features())
    }

    fn limits(&mut self) -> abi::SupportedLimits {
        self.limits
    }

    fn create_buffer(&mut self, descriptor: abi::BufferDescriptor) -> Option<abi::Buffer> {
        let (mapped, size) = (descriptor.mapped_at_creation, descriptor.size);
        let created = self.create(
            |gpu| &mut gpu.buffers,
            |gpu| {
                Ok(gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: convert::label(&descriptor.label),
                    size: descriptor.size,
                    usage: gpu.resolvable(wgpu::BufferUsages::from_bits_truncate(descriptor.usage)),
                    mapped_at_creation: descriptor.mapped_at_creation,
                }))
            },
        );
        if let Some(handle) = created
            && mapped
        {
            self.maps.mapped_at_creation(handle, size);
        }
        created.map(abi::Buffer)
    }

    fn create_texture(&mut self, descriptor: abi::TextureDescriptor) -> Option<abi::Texture> {
        let view_formats: Vec<_> = descriptor
            .view_formats
            .iter()
            .map(|format| convert::texture_format(*format))
            .collect();
        self.create(
            |gpu| &mut gpu.textures,
            |gpu| {
                Ok(gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: convert::label(&descriptor.label),
                    size: convert::extent(descriptor.size),
                    mip_level_count: descriptor.mip_level_count,
                    sample_count: descriptor.sample_count,
                    dimension: convert::texture_dimension(descriptor.dimension),
                    format: convert::texture_format(descriptor.format),
                    usage: wgpu::TextureUsages::from_bits_truncate(descriptor.usage),
                    view_formats: &view_formats,
                }))
            },
        )
        .map(abi::Texture)
    }

    fn create_sampler(&mut self, descriptor: abi::SamplerDescriptor) -> Option<abi::Sampler> {
        self.create(
            |gpu| &mut gpu.samplers,
            |gpu| {
                Ok(gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                    label: convert::label(&descriptor.label),
                    address_mode_u: convert::address_mode(descriptor.address_mode_u),
                    address_mode_v: convert::address_mode(descriptor.address_mode_v),
                    address_mode_w: convert::address_mode(descriptor.address_mode_w),
                    mag_filter: convert::filter_mode(descriptor.mag_filter),
                    min_filter: convert::filter_mode(descriptor.min_filter),
                    mipmap_filter: convert::mipmap_filter_mode(descriptor.mipmap_filter),
                    lod_min_clamp: descriptor.lod_min_clamp,
                    lod_max_clamp: descriptor.lod_max_clamp,
                    compare: descriptor.compare.map(convert::compare_function),
                    anisotropy_clamp: descriptor.anisotropy_clamp,
                    border_color: descriptor.border_color.map(convert::border_color),
                }))
            },
        )
        .map(abi::Sampler)
    }

    fn create_bind_group_layout(
        &mut self,
        descriptor: abi::BindGroupLayoutDescriptor,
    ) -> Option<abi::BindGroupLayout> {
        let entries: Vec<_> = descriptor
            .entries
            .iter()
            .map(|entry| wgpu::BindGroupLayoutEntry {
                binding: entry.binding,
                visibility: wgpu::ShaderStages::from_bits_truncate(entry.visibility),
                ty: convert::binding_type(entry.r#type),
                count: entry.count.and_then(std::num::NonZeroU32::new),
            })
            .collect();
        self.create(
            |gpu| &mut gpu.group_layouts,
            |gpu| {
                Ok(gpu
                    .device
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: convert::label(&descriptor.label),
                        entries: &entries,
                    }))
            },
        )
        .map(abi::BindGroupLayout)
    }

    fn create_bind_group(
        &mut self,
        descriptor: abi::BindGroupDescriptor,
    ) -> Option<abi::BindGroup> {
        self.create(
            |gpu| &mut gpu.groups,
            |gpu| gpu.try_create_bind_group(&descriptor),
        )
        .map(abi::BindGroup)
    }

    fn create_pipeline_layout(
        &mut self,
        descriptor: abi::PipelineLayoutDescriptor,
    ) -> Option<abi::PipelineLayout> {
        self.create(
            |gpu| &mut gpu.pipeline_layouts,
            |gpu| {
                let layouts = descriptor
                    .bind_group_layouts
                    .iter()
                    .map(|layout| {
                        layout
                            .map(|layout| {
                                gpu.group_layouts
                                    .get(layout.0, "bind group layout")
                                    .cloned()
                            })
                            .transpose()
                    })
                    .collect::<Outcome<Vec<_>>>()?;
                let borrowed: Vec<_> = layouts.iter().map(Option::as_ref).collect();
                Ok(gpu
                    .device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: convert::label(&descriptor.label),
                        bind_group_layouts: &borrowed,
                        immediate_size: 0,
                    }))
            },
        )
        .map(abi::PipelineLayout)
    }

    fn create_shader_module(
        &mut self,
        descriptor: abi::ShaderModuleDescriptor,
    ) -> Option<abi::ShaderModule> {
        self.create(
            |gpu| &mut gpu.modules,
            |gpu| {
                Ok(gpu
                    .device
                    .create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: convert::label(&descriptor.label),
                        source: wgpu::ShaderSource::Wgsl(descriptor.wgsl.as_str().into()),
                    }))
            },
        )
        .map(abi::ShaderModule)
    }

    fn create_render_pipeline(
        &mut self,
        descriptor: abi::RenderPipelineDescriptor,
    ) -> Option<abi::RenderPipeline> {
        self.create(
            |gpu| &mut gpu.pipelines,
            |gpu| gpu.try_create_render_pipeline(&descriptor),
        )
        .map(abi::RenderPipeline)
    }

    fn create_compute_pipeline(
        &mut self,
        descriptor: abi::ComputePipelineDescriptor,
    ) -> Option<abi::ComputePipeline> {
        self.create(
            |gpu| &mut gpu.compute_pipelines,
            |gpu| gpu.try_create_compute_pipeline(&descriptor),
        )
        .map(abi::ComputePipeline)
    }

    fn create_command_encoder(
        &mut self,
        descriptor: abi::CommandEncoderDescriptor,
    ) -> Option<abi::CommandEncoder> {
        self.create(
            |gpu| &mut gpu.encoders,
            |gpu| {
                Ok(gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: convert::label(&descriptor.label),
                    }))
            },
        )
        .map(abi::CommandEncoder)
    }

    fn write_buffer(&mut self, buffer: abi::Buffer, offset: u64, data: &[u8]) {
        self.guard(|gpu| {
            gpu.buffers
                .get(buffer.0, "buffer")
                .map(|buffer| gpu.queue.write_buffer(buffer, offset, data))
        });
    }

    fn write_texture(
        &mut self,
        destination: abi::TexelCopyTextureInfo,
        data: &[u8],
        layout: abi::TexelCopyBufferLayout,
        size: abi::Extent3d,
    ) {
        self.guard(|gpu| {
            let texture = gpu.textures.get(destination.texture.0, "texture")?;
            gpu.queue.write_texture(
                texel_copy_texture(texture, destination),
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: layout.offset,
                    bytes_per_row: layout.bytes_per_row,
                    rows_per_image: layout.rows_per_image,
                },
                convert::extent(size),
            );
            Ok(())
        });
    }

    fn submit(&mut self, command_buffers: &[abi::CommandBuffer]) {
        self.guard(|gpu| {
            let mut buffers = Vec::with_capacity(command_buffers.len());
            for buffer in command_buffers {
                buffers.push(gpu.command_buffers.take(buffer.0, "command buffer")?);
            }
            gpu.queue.submit(buffers);
            Ok(())
        });
    }

    fn poll(&mut self, wait: bool) {
        let poll = match wait {
            true => wgpu::PollType::wait_indefinitely(),
            false => wgpu::PollType::Poll,
        };
        if let Err(error) = self.device.poll(poll) {
            self.report(format!("the gpu could not be polled: {error}"));
        }
    }

    fn import_texture(&mut self, image: u64) -> Option<abi::Texture> {
        let texture = self
            .imports
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .get(&image)
            .cloned();
        match texture {
            Some(texture) => Some(abi::Texture(self.textures.insert(texture))),
            None => {
                self.textures.skip();
                None
            }
        }
    }

    fn take_error(&mut self) -> Option<String> {
        Gpu::take_error(self)
    }

    fn buffer_map_async(&mut self, this: abi::Buffer, mode: abi::MapMode, offset: u64, size: u64) {
        self.guard(|gpu| {
            let buffer = gpu.buffers.get(this.0, "buffer")?.clone();
            let end = offset
                .checked_add(size)
                .filter(|end| size > 0 && *end <= buffer.size())
                .ok_or_else(|| {
                    format!(
                        "a map of {offset}..{} is empty or runs past the end of its buffer",
                        offset.saturating_add(size)
                    )
                })?;
            gpu.maps
                .map(&gpu.device, &buffer, this.0, mode, offset..end)
        });
    }

    fn buffer_map_state(&mut self, this: abi::Buffer) -> abi::MapState {
        self.maps.state(this.0)
    }

    fn buffer_read_mapped(&mut self, this: abi::Buffer, offset: u64, size: u64) -> Vec<u8> {
        let read = self
            .mapped_range(this, offset, size, false)
            .map(|(buffer, range)| match range.is_empty() {
                true => Vec::new(),
                false => buffer.slice(range).get_mapped_range().to_vec(),
            });
        self.fail(read, Vec::new())
    }

    fn buffer_write_mapped(&mut self, this: abi::Buffer, offset: u64, data: &[u8]) {
        let written = self
            .mapped_range(this, offset, data.len() as u64, true)
            .map(|(buffer, range)| {
                if !range.is_empty() {
                    buffer
                        .slice(range)
                        .get_mapped_range_mut()
                        .copy_from_slice(data);
                }
            });
        self.fail(written, ());
    }

    fn buffer_unmap(&mut self, this: abi::Buffer) {
        self.maps.forget(this.0);
        self.guard(|gpu| gpu.buffers.get(this.0, "buffer").map(wgpu::Buffer::unmap));
    }

    fn buffer_drop(&mut self, this: abi::Buffer) {
        self.maps.forget(this.0);
        self.buffers.remove(this.0);
    }

    fn texture_create_view(
        &mut self,
        this: abi::Texture,
        descriptor: abi::TextureViewDescriptor,
    ) -> Option<abi::TextureView> {
        self.create(
            |gpu| &mut gpu.views,
            |gpu| {
                let texture = gpu.textures.get(this.0, "texture")?;
                Ok(texture.create_view(&wgpu::TextureViewDescriptor {
                    label: convert::label(&descriptor.label),
                    format: descriptor.format.map(convert::texture_format),
                    dimension: descriptor.dimension.map(convert::texture_view_dimension),
                    usage: None,
                    aspect: convert::texture_aspect(descriptor.aspect),
                    base_mip_level: descriptor.base_mip_level,
                    mip_level_count: descriptor.mip_level_count,
                    base_array_layer: descriptor.base_array_layer,
                    array_layer_count: descriptor.array_layer_count,
                }))
            },
        )
        .map(abi::TextureView)
    }

    fn texture_describe(&mut self, this: abi::Texture) -> Option<abi::TextureInfo> {
        let described = self.textures.get(this.0, "texture").and_then(|texture| {
            let unsupported = || format!("texture {} has a format plugins cannot use", this.0);
            Ok(abi::TextureInfo {
                size: abi::Extent3d {
                    width: texture.width(),
                    height: texture.height(),
                    depth_or_array_layers: texture.depth_or_array_layers(),
                },
                mip_level_count: texture.mip_level_count(),
                sample_count: texture.sample_count(),
                dimension: convert::abi_texture_dimension(texture.dimension())
                    .ok_or_else(unsupported)?,
                format: convert::abi_texture_format(texture.format()).ok_or_else(unsupported)?,
                usage: texture.usage().bits(),
            })
        });
        self.fail(described.map(Some), None)
    }

    fn texture_drop(&mut self, this: abi::Texture) {
        self.textures.remove(this.0);
    }

    fn texture_view_drop(&mut self, this: abi::TextureView) {
        self.views.remove(this.0);
    }

    fn sampler_drop(&mut self, this: abi::Sampler) {
        self.samplers.remove(this.0);
    }

    fn bind_group_layout_drop(&mut self, this: abi::BindGroupLayout) {
        self.group_layouts.remove(this.0);
    }

    fn bind_group_drop(&mut self, this: abi::BindGroup) {
        self.groups.remove(this.0);
    }

    fn pipeline_layout_drop(&mut self, this: abi::PipelineLayout) {
        self.pipeline_layouts.remove(this.0);
    }

    fn shader_module_drop(&mut self, this: abi::ShaderModule) {
        self.modules.remove(this.0);
    }

    fn render_pipeline_get_bind_group_layout(
        &mut self,
        this: abi::RenderPipeline,
        index: u32,
    ) -> Option<abi::BindGroupLayout> {
        self.create(
            |gpu| &mut gpu.group_layouts,
            |gpu| {
                let pipeline = gpu.pipelines.get(this.0, "render pipeline")?;
                Ok(pipeline.get_bind_group_layout(index))
            },
        )
        .map(abi::BindGroupLayout)
    }

    fn render_pipeline_drop(&mut self, this: abi::RenderPipeline) {
        self.pipelines.remove(this.0);
    }

    fn command_encoder_begin_render_pass(
        &mut self,
        this: abi::CommandEncoder,
        descriptor: abi::RenderPassDescriptor,
    ) -> Option<abi::RenderPass> {
        self.create(
            |gpu| &mut gpu.passes,
            |gpu| gpu.try_begin_render_pass(this, &descriptor),
        )
        .map(abi::RenderPass)
    }

    fn command_encoder_begin_compute_pass(
        &mut self,
        this: abi::CommandEncoder,
        descriptor: abi::ComputePassDescriptor,
    ) -> Option<abi::ComputePass> {
        self.create(
            |gpu| &mut gpu.compute_passes,
            |gpu| gpu.try_begin_compute_pass(this, &descriptor),
        )
        .map(abi::ComputePass)
    }

    fn command_encoder_copy_buffer_to_buffer(
        &mut self,
        this: abi::CommandEncoder,
        source: abi::Buffer,
        source_offset: u64,
        destination: abi::Buffer,
        destination_offset: u64,
        size: Option<u64>,
    ) {
        self.encode(this, |gpu| {
            let from = gpu.buffers.get(source.0, "buffer")?.clone();
            let to = gpu.buffers.get(destination.0, "buffer")?.clone();
            Ok(Box::new(move |encoder| {
                encoder.copy_buffer_to_buffer(&from, source_offset, &to, destination_offset, size)
            }))
        });
    }

    fn command_encoder_copy_buffer_to_texture(
        &mut self,
        this: abi::CommandEncoder,
        source: abi::TexelCopyBufferInfo,
        destination: abi::TexelCopyTextureInfo,
        size: abi::Extent3d,
    ) {
        self.encode(this, |gpu| {
            let from = gpu.buffers.get(source.buffer.0, "buffer")?.clone();
            let to = gpu.textures.get(destination.texture.0, "texture")?.clone();
            Ok(Box::new(move |encoder| {
                encoder.copy_buffer_to_texture(
                    texel_copy_buffer(&from, source.layout),
                    texel_copy_texture(&to, destination),
                    convert::extent(size),
                )
            }))
        });
    }

    fn command_encoder_copy_texture_to_buffer(
        &mut self,
        this: abi::CommandEncoder,
        source: abi::TexelCopyTextureInfo,
        destination: abi::TexelCopyBufferInfo,
        size: abi::Extent3d,
    ) {
        self.encode(this, |gpu| {
            let from = gpu.textures.get(source.texture.0, "texture")?.clone();
            let to = gpu.buffers.get(destination.buffer.0, "buffer")?.clone();
            Ok(Box::new(move |encoder| {
                encoder.copy_texture_to_buffer(
                    texel_copy_texture(&from, source),
                    texel_copy_buffer(&to, destination.layout),
                    convert::extent(size),
                )
            }))
        });
    }

    fn command_encoder_copy_texture_to_texture(
        &mut self,
        this: abi::CommandEncoder,
        source: abi::TexelCopyTextureInfo,
        destination: abi::TexelCopyTextureInfo,
        size: abi::Extent3d,
    ) {
        self.encode(this, |gpu| {
            let from = gpu.textures.get(source.texture.0, "texture")?.clone();
            let to = gpu.textures.get(destination.texture.0, "texture")?.clone();
            Ok(Box::new(move |encoder| {
                encoder.copy_texture_to_texture(
                    texel_copy_texture(&from, source),
                    texel_copy_texture(&to, destination),
                    convert::extent(size),
                )
            }))
        });
    }

    fn command_encoder_clear_buffer(
        &mut self,
        this: abi::CommandEncoder,
        buffer: abi::Buffer,
        offset: u64,
        size: Option<u64>,
    ) {
        self.encode(this, |gpu| {
            let buffer = gpu.buffers.get(buffer.0, "buffer")?.clone();
            Ok(Box::new(move |encoder| {
                encoder.clear_buffer(&buffer, offset, size)
            }))
        });
    }

    fn command_encoder_finish(&mut self, this: abi::CommandEncoder) -> Option<abi::CommandBuffer> {
        self.create(
            |gpu| &mut gpu.command_buffers,
            |gpu| {
                gpu.encoders
                    .take(this.0, "command encoder")
                    .map(wgpu::CommandEncoder::finish)
            },
        )
        .map(abi::CommandBuffer)
    }

    fn command_encoder_drop(&mut self, this: abi::CommandEncoder) {
        self.encoders.remove(this.0);
    }

    fn command_buffer_drop(&mut self, this: abi::CommandBuffer) {
        self.command_buffers.remove(this.0);
    }

    fn render_pass_set_pipeline(&mut self, this: abi::RenderPass, pipeline: abi::RenderPipeline) {
        let result = self
            .pipelines
            .get(pipeline.0, "render pipeline")
            .cloned()
            .and_then(|pipeline| {
                self.passes
                    .get_mut(this.0, "render pass")
                    .map(|pass| pass.set_pipeline(&pipeline))
            });
        self.fail(result, ());
    }

    fn render_pass_set_bind_group(
        &mut self,
        this: abi::RenderPass,
        index: u32,
        bind_group: Option<abi::BindGroup>,
        dynamic_offsets: &[u32],
    ) {
        let result = bind_group
            .map(|group| self.groups.get(group.0, "bind group").cloned())
            .transpose()
            .and_then(|group| {
                self.passes
                    .get_mut(this.0, "render pass")
                    .map(|pass| pass.set_bind_group(index, group.as_ref(), dynamic_offsets))
            });
        self.fail(result, ());
    }

    fn render_pass_set_index_buffer(
        &mut self,
        this: abi::RenderPass,
        buffer: abi::Buffer,
        format: abi::IndexFormat,
        offset: u64,
        size: Option<u64>,
    ) {
        let result = self
            .buffers
            .get(buffer.0, "buffer")
            .cloned()
            .and_then(|buffer| {
                let slice = slice(&buffer, offset, size)?;
                self.passes
                    .get_mut(this.0, "render pass")
                    .map(|pass| pass.set_index_buffer(slice, convert::index_format(format)))
            });
        self.fail(result, ());
    }

    fn render_pass_set_vertex_buffer(
        &mut self,
        this: abi::RenderPass,
        slot: u32,
        buffer: abi::Buffer,
        offset: u64,
        size: Option<u64>,
    ) {
        let result = self
            .buffers
            .get(buffer.0, "buffer")
            .cloned()
            .and_then(|buffer| {
                let slice = slice(&buffer, offset, size)?;
                self.passes
                    .get_mut(this.0, "render pass")
                    .map(|pass| pass.set_vertex_buffer(slot, slice))
            });
        self.fail(result, ());
    }

    fn render_pass_set_viewport(
        &mut self,
        this: abi::RenderPass,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        min_depth: f32,
        max_depth: f32,
    ) {
        self.pass(this, |pass| {
            pass.set_viewport(x, y, width, height, min_depth, max_depth)
        });
    }

    fn render_pass_set_scissor_rect(
        &mut self,
        this: abi::RenderPass,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        self.pass(this, |pass| pass.set_scissor_rect(x, y, width, height));
    }

    fn render_pass_set_blend_constant(&mut self, this: abi::RenderPass, color: abi::Color) {
        self.pass(this, |pass| pass.set_blend_constant(convert::color(color)));
    }

    fn render_pass_set_stencil_reference(&mut self, this: abi::RenderPass, reference: u32) {
        self.pass(this, |pass| pass.set_stencil_reference(reference));
    }

    fn render_pass_draw(
        &mut self,
        this: abi::RenderPass,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        self.pass(this, |pass| {
            pass.draw(
                first_vertex..first_vertex.saturating_add(vertex_count),
                first_instance..first_instance.saturating_add(instance_count),
            )
        });
    }

    fn render_pass_draw_indexed(
        &mut self,
        this: abi::RenderPass,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    ) {
        self.pass(this, |pass| {
            pass.draw_indexed(
                first_index..first_index.saturating_add(index_count),
                base_vertex,
                first_instance..first_instance.saturating_add(instance_count),
            )
        });
    }

    fn render_pass_end(&mut self, this: abi::RenderPass) {
        self.guard(|gpu| gpu.passes.take(this.0, "render pass").map(drop));
    }

    fn render_pass_drop(&mut self, this: abi::RenderPass) {
        self.passes.remove(this.0);
    }

    fn compute_pipeline_get_bind_group_layout(
        &mut self,
        this: abi::ComputePipeline,
        index: u32,
    ) -> Option<abi::BindGroupLayout> {
        self.create(
            |gpu| &mut gpu.group_layouts,
            |gpu| {
                let pipeline = gpu.compute_pipelines.get(this.0, "compute pipeline")?;
                Ok(pipeline.get_bind_group_layout(index))
            },
        )
        .map(abi::BindGroupLayout)
    }

    fn compute_pipeline_drop(&mut self, this: abi::ComputePipeline) {
        self.compute_pipelines.remove(this.0);
    }

    fn compute_pass_set_pipeline(
        &mut self,
        this: abi::ComputePass,
        pipeline: abi::ComputePipeline,
    ) {
        let result = self
            .compute_pipelines
            .get(pipeline.0, "compute pipeline")
            .cloned()
            .and_then(|pipeline| {
                self.compute_passes
                    .get_mut(this.0, "compute pass")
                    .map(|pass| pass.set_pipeline(&pipeline))
            });
        self.fail(result, ());
    }

    fn compute_pass_set_bind_group(
        &mut self,
        this: abi::ComputePass,
        index: u32,
        bind_group: Option<abi::BindGroup>,
        dynamic_offsets: &[u32],
    ) {
        let result = bind_group
            .map(|group| self.groups.get(group.0, "bind group").cloned())
            .transpose()
            .and_then(|group| {
                self.compute_passes
                    .get_mut(this.0, "compute pass")
                    .map(|pass| pass.set_bind_group(index, group.as_ref(), dynamic_offsets))
            });
        self.fail(result, ());
    }

    fn compute_pass_dispatch_workgroups(&mut self, this: abi::ComputePass, x: u32, y: u32, z: u32) {
        self.compute_pass(this, |pass| pass.dispatch_workgroups(x, y, z));
    }

    fn compute_pass_dispatch_workgroups_indirect(
        &mut self,
        this: abi::ComputePass,
        buffer: abi::Buffer,
        offset: u64,
    ) {
        let result = self.buffers.get(buffer.0, "buffer").cloned();
        match result {
            Ok(buffer) => self.compute_pass(this, |pass| {
                pass.dispatch_workgroups_indirect(&buffer, offset)
            }),
            Err(message) => self.report(message),
        }
    }

    fn compute_pass_end(&mut self, this: abi::ComputePass) {
        self.guard(|gpu| gpu.compute_passes.take(this.0, "compute pass").map(drop));
    }

    fn compute_pass_drop(&mut self, this: abi::ComputePass) {
        self.compute_passes.remove(this.0);
    }

    fn surface_new(&mut self, id: u32) -> abi::Surface {
        abi::Surface(self.surface_ids.insert(id))
    }

    fn surface_configure(&mut self, this: abi::Surface, configuration: abi::SurfaceConfiguration) {
        let Some(surface) = self.surface_id(this) else {
            return;
        };
        let abi::SurfaceConfiguration {
            width,
            height,
            format,
        } = configuration;
        let limit = self.device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > limit || height > limit {
            self.report(format!(
                "surface {surface} asked for a {width} by {height} target, which this device cannot make"
            ));
            return;
        }
        let format = convert::texture_format(format);
        let matches = self.surfaces.get(&surface).is_some_and(|existing| {
            let (texture, _) = existing.shown();
            existing.textures.len() == 2
                && texture.width() == width
                && texture.height() == height
                && texture.format() == format
        });
        if matches {
            return;
        }
        let texture = || {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("plugin surface"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: SURFACE_USAGE,
                view_formats: &[],
            })
        };
        let textures = vec![texture(), texture()];
        self.insert_surface(surface, textures);
    }

    fn surface_acquire(&mut self, this: abi::Surface) -> Option<abi::Texture> {
        let surface = self.surface_id(this)?;
        let Some(target) = self.surfaces.get(&surface) else {
            self.report(format!("surface {surface} has no target texture"));
            self.textures.skip();
            return None;
        };
        let texture = target.textures[target.drawn].0.clone();
        Some(abi::Texture(self.textures.insert(texture)))
    }

    fn surface_age(&mut self, this: abi::Surface) -> u32 {
        self.surface_id(this)
            .and_then(|surface| self.surfaces.get(&surface))
            .map_or(0, |target| target.ages.age())
    }

    fn surface_present(&mut self, this: abi::Surface) {
        let Some(surface) = self.surface_id(this) else {
            return;
        };
        if let Some(target) = self.surfaces.get_mut(&surface) {
            target.drawn = (target.drawn + 1) % target.textures.len();
            target.ages.present();
        }
        self.presented.push(surface);
    }

    fn surface_drop(&mut self, this: abi::Surface) {
        if let Ok(surface) = self.surface_ids.take(this.0, "surface") {
            self.detach_surface(surface);
        }
    }

    fn create_query_set(&mut self, descriptor: abi::QuerySetDescriptor) -> Option<abi::QuerySet> {
        self.create(
            |gpu| &mut gpu.query_sets,
            |gpu| {
                let (ty, timestamps) = match descriptor.r#type {
                    abi::QueryType::Occlusion => (wgpu::QueryType::Occlusion, false),
                    abi::QueryType::Timestamp => (wgpu::QueryType::Timestamp, true),
                };
                Ok(QuerySet {
                    set: gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
                        label: convert::label(&descriptor.label),
                        ty,
                        count: descriptor.count,
                    }),
                    timestamps,
                })
            },
        )
        .map(abi::QuerySet)
    }

    fn query_set_drop(&mut self, this: abi::QuerySet) {
        self.query_sets.remove(this.0);
    }

    fn command_encoder_resolve_query_set(
        &mut self,
        this: abi::CommandEncoder,
        query_set: abi::QuerySet,
        first_query: u32,
        query_count: u32,
        destination: abi::Buffer,
        destination_offset: u64,
    ) {
        self.encode(this, |gpu| {
            let set = gpu.query_sets.get(query_set.0, "query set")?;
            let (queries, timestamps) = (set.set.clone(), set.timestamps);
            let to = gpu.buffers.get(destination.0, "buffer")?.clone();
            let coarsen = timestamps && Coarsening::fits(&to, destination_offset, query_count);
            let coarsening = coarsen.then(|| {
                let pipeline = gpu.coarsening().clone();
                (pipeline, gpu.device.clone())
            });
            Ok(Box::new(move |encoder| {
                let queries_range = first_query..first_query.saturating_add(query_count);
                encoder.resolve_query_set(&queries, queries_range, &to, destination_offset);
                if let Some((coarsening, device)) = coarsening {
                    coarsening.record(&device, encoder, &to, destination_offset, query_count);
                }
            }))
        });
    }

    fn render_pass_draw_indirect(
        &mut self,
        this: abi::RenderPass,
        indirect_buffer: abi::Buffer,
        indirect_offset: u64,
    ) {
        let result = self.buffers.get(indirect_buffer.0, "buffer").cloned();
        if let Some(buffer) = self.fail(result.map(Some), None) {
            self.pass(this, |pass| pass.draw_indirect(&buffer, indirect_offset));
        }
    }

    fn render_pass_draw_indexed_indirect(
        &mut self,
        this: abi::RenderPass,
        indirect_buffer: abi::Buffer,
        indirect_offset: u64,
    ) {
        let result = self.buffers.get(indirect_buffer.0, "buffer").cloned();
        if let Some(buffer) = self.fail(result.map(Some), None) {
            self.pass(this, |pass| {
                pass.draw_indexed_indirect(&buffer, indirect_offset)
            });
        }
    }

    fn render_pass_begin_occlusion_query(&mut self, this: abi::RenderPass, query_index: u32) {
        self.pass(this, |pass| pass.begin_occlusion_query(query_index));
    }

    fn render_pass_end_occlusion_query(&mut self, this: abi::RenderPass) {
        self.pass(this, |pass| pass.end_occlusion_query());
    }

    fn render_pass_execute_bundles(
        &mut self,
        this: abi::RenderPass,
        bundles: &[abi::RenderBundle],
    ) {
        let found = bundles
            .iter()
            .map(|bundle| self.bundles.get(bundle.0, "render bundle").cloned())
            .collect::<Outcome<Vec<_>>>();
        if let Some(bundles) = self.fail(found.map(Some), None) {
            self.pass(this, |pass| pass.execute_bundles(&bundles));
        }
    }

    fn create_render_bundle_encoder(
        &mut self,
        descriptor: abi::RenderBundleEncoderDescriptor,
    ) -> Option<abi::RenderBundleEncoder> {
        self.create(
            |gpu| &mut gpu.bundle_encoders,
            |_| BundleEncoder::new(descriptor),
        )
        .map(abi::RenderBundleEncoder)
    }

    fn render_bundle_encoder_set_pipeline(
        &mut self,
        this: abi::RenderBundleEncoder,
        pipeline: abi::RenderPipeline,
    ) {
        self.bundle(this, |gpu| {
            let pipeline = gpu.pipelines.get(pipeline.0, "render pipeline")?;
            Ok(Command::Pipeline(pipeline.clone()))
        });
    }

    fn render_bundle_encoder_set_bind_group(
        &mut self,
        this: abi::RenderBundleEncoder,
        index: u32,
        bind_group: Option<abi::BindGroup>,
        dynamic_offsets: &[u32],
    ) {
        self.bundle(this, |gpu| {
            let group = bind_group
                .map(|group| gpu.groups.get(group.0, "bind group").cloned())
                .transpose()?;
            Ok(Command::BindGroup(index, group, dynamic_offsets.to_vec()))
        });
    }

    fn render_bundle_encoder_set_index_buffer(
        &mut self,
        this: abi::RenderBundleEncoder,
        buffer: abi::Buffer,
        format: abi::IndexFormat,
        offset: u64,
        size: Option<u64>,
    ) {
        self.bundle(this, |gpu| {
            let buffer = gpu.buffers.get(buffer.0, "buffer")?.clone();
            let format = convert::index_format(format);
            Ok(Command::IndexBuffer(buffer, format, offset, size))
        });
    }

    fn render_bundle_encoder_set_vertex_buffer(
        &mut self,
        this: abi::RenderBundleEncoder,
        slot: u32,
        buffer: abi::Buffer,
        offset: u64,
        size: Option<u64>,
    ) {
        self.bundle(this, |gpu| {
            let buffer = gpu.buffers.get(buffer.0, "buffer")?.clone();
            Ok(Command::VertexBuffer(slot, buffer, offset, size))
        });
    }

    fn render_bundle_encoder_draw(
        &mut self,
        this: abi::RenderBundleEncoder,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        self.bundle(this, |_| {
            Ok(Command::Draw(
                first_vertex..first_vertex.saturating_add(vertex_count),
                first_instance..first_instance.saturating_add(instance_count),
            ))
        });
    }

    fn render_bundle_encoder_draw_indexed(
        &mut self,
        this: abi::RenderBundleEncoder,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        base_vertex: i32,
        first_instance: u32,
    ) {
        self.bundle(this, |_| {
            Ok(Command::DrawIndexed(
                first_index..first_index.saturating_add(index_count),
                base_vertex,
                first_instance..first_instance.saturating_add(instance_count),
            ))
        });
    }

    fn render_bundle_encoder_draw_indirect(
        &mut self,
        this: abi::RenderBundleEncoder,
        indirect_buffer: abi::Buffer,
        indirect_offset: u64,
    ) {
        self.bundle(this, |gpu| {
            let buffer = gpu.buffers.get(indirect_buffer.0, "buffer")?.clone();
            Ok(Command::DrawIndirect(buffer, indirect_offset))
        });
    }

    fn render_bundle_encoder_draw_indexed_indirect(
        &mut self,
        this: abi::RenderBundleEncoder,
        indirect_buffer: abi::Buffer,
        indirect_offset: u64,
    ) {
        self.bundle(this, |gpu| {
            let buffer = gpu.buffers.get(indirect_buffer.0, "buffer")?.clone();
            Ok(Command::DrawIndexedIndirect(buffer, indirect_offset))
        });
    }

    fn render_bundle_encoder_finish(
        &mut self,
        this: abi::RenderBundleEncoder,
        descriptor: abi::RenderBundleDescriptor,
    ) -> Option<abi::RenderBundle> {
        self.create(
            |gpu| &mut gpu.bundles,
            |gpu| {
                gpu.bundle_encoders
                    .take(this.0, "render bundle encoder")?
                    .finish(&gpu.device, &descriptor)
            },
        )
        .map(abi::RenderBundle)
    }

    fn render_bundle_encoder_drop(&mut self, this: abi::RenderBundleEncoder) {
        self.bundle_encoders.remove(this.0);
    }

    fn render_bundle_drop(&mut self, this: abi::RenderBundle) {
        self.bundles.remove(this.0);
    }
}
