use std::collections::BTreeMap;

use be_wasm_gpu_abi::device as abi;
use be_wasm_gpu_abi::embed::webgpu::Host;
use be_wasm_gpu_abi::webgpu::webgpu as web;
use be_wasm_gpu_convert::allowed::Allowed;
use be_wasm_gpu_convert::{Convert, TryConvert, webgpu as convert};
use be_wasm_ops::local::{Promise, Sender, Settled};

use super::{Failure, Kind};
use crate::Gpu;
use crate::queries::QuerySet;
use crate::tables::Table;

/// A missing object, as WebGPU's use of an invalid one: a validation error.
pub(super) fn found<T>(result: Result<T, String>) -> Result<T, Failure> {
    result.map_err(Failure::validation)
}

impl Gpu {
    fn allowed(&self, device: u32) -> Allowed {
        Allowed {
            requested: self
                .device_has(device)
                .map_or(wgpu::Features::empty(), |(features, _)| features),
            offered: self.device.features(),
        }
    }

    /// Makes an object on an app device: an invalid one (a handle with nothing behind it, whose
    /// use is a validation error) when the device is lost or the create fails.
    pub(super) fn make<T>(
        &mut self,
        device: Option<u32>,
        kind: Kind,
        label: Option<&str>,
        table: fn(&mut Self) -> &mut Table<T>,
        act: impl FnOnce(&mut Self) -> Result<T, Failure>,
    ) -> u32 {
        let lost = device.is_some_and(|device| self.device_lost(device));
        let made = match lost {
            true => None,
            false => self.wrapped(device, act),
        };
        let handle = match made {
            Some(value) => table(self).insert(value),
            None => table(self).skip(),
        };
        self.own(kind, handle, device, label);
        handle
    }

    /// Runs a call that may fail on an app device, raising what it meets.
    pub(super) fn checked(&mut self, device: Option<u32>, act: impl FnOnce(&mut Self) -> Result<(), Failure>) {
        if device.is_some_and(|device| self.device_lost(device)) {
            return;
        }
        if let Err(failure) = act(self) {
            self.raise(device, failure);
        }
    }

    fn buffer_range(
        &self,
        buffer: u32,
        offset: Option<u64>,
        size: Option<u64>,
    ) -> Result<(wgpu::Buffer, u64, u64), Failure> {
        let found = found(self.buffers.get(buffer, "buffer"))?.clone();
        let offset = offset.unwrap_or(0);
        let size = match size {
            Some(size) => size,
            None => found.size().checked_sub(offset).ok_or_else(|| {
                Failure::validation(format!(
                    "offset {offset} is past the end of buffer {buffer}"
                ))
            })?,
        };
        Ok((found, offset, size))
    }

    fn pipeline_layout(&self, layout: web::GpuLayoutMode) -> Result<Option<wgpu::PipelineLayout>, Failure> {
        match layout {
            web::GpuLayoutMode::Auto => Ok(None),
            web::GpuLayoutMode::Specific(layout) => {
                found(self.pipeline_layouts.get(layout.0, "pipeline layout")).map(|layout| Some(layout.clone()))
            }
        }
    }

    fn constants(&self, record: Option<web::RecordGpuPipelineConstantValue>) -> Result<Vec<(String, f64)>, Failure> {
        match record {
            None => Ok(Vec::new()),
            Some(record) => found(self.web.constants.get(record.0, "pipeline constants"))
                .map(|constants| constants.iter().map(|(key, value)| (key.clone(), *value)).collect()),
        }
    }

    pub(super) fn try_render_pipeline(
        &mut self,
        device: u32,
        descriptor: &web::GpuRenderPipelineDescriptor,
    ) -> Result<wgpu::RenderPipeline, Failure> {
        let layout = self.pipeline_layout(descriptor.layout)?;
        let vertex_module = found(self.modules.get(descriptor.vertex.module.0, "shader module"))?.clone();
        let fragment_module = descriptor
            .fragment
            .as_ref()
            .map(|fragment| found(self.modules.get(fragment.module.0, "shader module")).cloned())
            .transpose()?;
        let vertex_constants = self.constants(descriptor.vertex.constants)?;
        let fragment_constants = match &descriptor.fragment {
            Some(fragment) => self.constants(fragment.constants)?,
            None => Vec::new(),
        };
        let layouts = descriptor.vertex.buffers.clone().unwrap_or_default();
        let attributes: Vec<Vec<wgpu::VertexAttribute>> = layouts
            .iter()
            .map(|layout| {
                layout
                    .iter()
                    .flat_map(|layout| &layout.attributes)
                    .map(|attribute| wgpu::VertexAttribute {
                        format: attribute.format.convert(),
                        offset: attribute.offset,
                        shader_location: attribute.shader_location,
                    })
                    .collect()
            })
            .collect();
        let buffers: Vec<_> = layouts
            .iter()
            .zip(&attributes)
            .map(|(layout, attributes)| {
                layout.as_ref().map(|layout| wgpu::VertexBufferLayout {
                    array_stride: layout.array_stride,
                    step_mode: layout
                        .step_mode
                        .map_or(wgpu::VertexStepMode::Vertex, Convert::convert),
                    attributes,
                })
            })
            .collect();
        let targets: Vec<Option<wgpu::ColorTargetState>> = descriptor
            .fragment
            .as_ref()
            .map(|fragment| {
                fragment
                    .targets
                    .iter()
                    .map(|target| {
                        target.as_ref().map(|target| wgpu::ColorTargetState {
                            format: target.format.convert(),
                            blend: target.blend.map(|blend| wgpu::BlendState {
                                color: blend.color.convert(),
                                alpha: blend.alpha.convert(),
                            }),
                            write_mask: target
                                .write_mask
                                .map_or(wgpu::ColorWrites::ALL, Convert::convert),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let primitive = descriptor.primitive.unwrap_or(web::GpuPrimitiveState {
            topology: None,
            strip_index_format: None,
            front_face: None,
            cull_mode: None,
            unclipped_depth: None,
        });
        let multisample = descriptor.multisample.unwrap_or(web::GpuMultisampleState {
            count: None,
            mask: None,
            alpha_to_coverage_enabled: None,
        });
        let vertex_constants: Vec<(&str, f64)> = vertex_constants.iter().map(|(key, value)| (key.as_str(), *value)).collect();
        let fragment_constants: Vec<(&str, f64)> = fragment_constants.iter().map(|(key, value)| (key.as_str(), *value)).collect();
        let wgpu_descriptor = wgpu::RenderPipelineDescriptor {
            label: descriptor.label.as_deref(),
            layout: layout.as_ref(),
            vertex: wgpu::VertexState {
                module: &vertex_module,
                entry_point: descriptor.vertex.entry_point.as_deref(),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &vertex_constants,
                    ..Default::default()
                },
                buffers: &buffers,
            },
            primitive: wgpu::PrimitiveState {
                topology: primitive
                    .topology
                    .map_or(wgpu::PrimitiveTopology::TriangleList, Convert::convert),
                strip_index_format: primitive.strip_index_format.map(Convert::convert),
                front_face: primitive.front_face.map_or(wgpu::FrontFace::Ccw, Convert::convert),
                cull_mode: match primitive.cull_mode {
                    None | Some(web::GpuCullMode::None) => None,
                    Some(web::GpuCullMode::Front) => Some(wgpu::Face::Front),
                    Some(web::GpuCullMode::Back) => Some(wgpu::Face::Back),
                },
                unclipped_depth: primitive.unclipped_depth.unwrap_or(false),
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: descriptor.depth_stencil.map(|state| wgpu::DepthStencilState {
                format: state.format.convert(),
                depth_write_enabled: state.depth_write_enabled,
                depth_compare: state.depth_compare.map(Convert::convert),
                stencil: wgpu::StencilState {
                    front: state.stencil_front.convert(),
                    back: state.stencil_back.convert(),
                    read_mask: state.stencil_read_mask.unwrap_or(u32::MAX),
                    write_mask: state.stencil_write_mask.unwrap_or(u32::MAX),
                },
                bias: wgpu::DepthBiasState {
                    constant: state.depth_bias.unwrap_or(0),
                    slope_scale: state.depth_bias_slope_scale.unwrap_or(0.0),
                    clamp: state.depth_bias_clamp.unwrap_or(0.0),
                },
            }),
            multisample: wgpu::MultisampleState {
                count: multisample.count.unwrap_or(1),
                mask: u64::from(multisample.mask.unwrap_or(u32::MAX)),
                alpha_to_coverage_enabled: multisample.alpha_to_coverage_enabled.unwrap_or(false),
            },
            fragment: fragment_module.as_ref().map(|module| wgpu::FragmentState {
                module,
                entry_point: descriptor
                    .fragment
                    .as_ref()
                    .and_then(|fragment| fragment.entry_point.as_deref()),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &fragment_constants,
                    ..Default::default()
                },
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        };
        self.allowed(device)
            .render_pipeline(&wgpu_descriptor)
            .map_err(Failure::validation)?;
        Ok(self.device.create_render_pipeline(&wgpu_descriptor))
    }

    pub(super) fn try_compute_pipeline(
        &mut self,
        descriptor: &web::GpuComputePipelineDescriptor,
    ) -> Result<wgpu::ComputePipeline, Failure> {
        let layout = self.pipeline_layout(descriptor.layout)?;
        let module = found(self.modules.get(descriptor.compute.module.0, "shader module"))?.clone();
        let constants = self.constants(descriptor.compute.constants)?;
        let constants: Vec<(&str, f64)> = constants.iter().map(|(key, value)| (key.as_str(), *value)).collect();
        Ok(self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: descriptor.label.as_deref(),
            layout: layout.as_ref(),
            module: &module,
            entry_point: descriptor.compute.entry_point.as_deref(),
            compilation_options: wgpu::PipelineCompilationOptions {
                constants: &constants,
                ..Default::default()
            },
            cache: None,
        }))
    }

    fn try_bind_group(&self, descriptor: &web::GpuBindGroupDescriptor) -> Result<wgpu::BindGroup, Failure> {
        let layout = found(self.group_layouts.get(descriptor.layout.0, "bind group layout"))?.clone();
        enum Held {
            Buffer(wgpu::Buffer, u64, Option<u64>),
            Sampler(wgpu::Sampler),
            View(wgpu::TextureView),
        }
        let mut held = Vec::with_capacity(descriptor.entries.len());
        for entry in &descriptor.entries {
            held.push(match entry.resource {
                web::GpuBindingResource::GpuBuffer(buffer) => {
                    Held::Buffer(found(self.buffers.get(buffer.0, "buffer"))?.clone(), 0, None)
                }
                web::GpuBindingResource::GpuBufferBinding(binding) => Held::Buffer(
                    found(self.buffers.get(binding.buffer.0, "buffer"))?.clone(),
                    binding.offset.unwrap_or(0),
                    binding.size,
                ),
                web::GpuBindingResource::GpuSampler(sampler) => {
                    Held::Sampler(found(self.samplers.get(sampler.0, "sampler"))?.clone())
                }
                web::GpuBindingResource::GpuTextureView(view) => {
                    Held::View(found(self.views.get(view.0, "texture view"))?.clone())
                }
                // A texture binds as its default view, as WebGPU's `GPUBindingResource` has it.
                web::GpuBindingResource::GpuTexture(texture) => Held::View(
                    found(self.textures.get(texture.0, "texture"))?
                        .create_view(&wgpu::TextureViewDescriptor::default()),
                ),
            });
        }
        let entries: Vec<wgpu::BindGroupEntry<'_>> = descriptor
            .entries
            .iter()
            .zip(&held)
            .map(|(entry, held)| wgpu::BindGroupEntry {
                binding: entry.binding,
                resource: match held {
                    Held::Buffer(buffer, offset, size) => wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: *offset,
                        size: size.and_then(wgpu::BufferSize::new),
                    }),
                    Held::Sampler(sampler) => wgpu::BindingResource::Sampler(sampler),
                    Held::View(view) => wgpu::BindingResource::TextureView(view),
                },
            })
            .collect();
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: descriptor.label.as_deref(),
            layout: &layout,
            entries: &entries,
        }))
    }

    fn texture_of<T>(&self, texture: web::GpuTexture, read: impl FnOnce(&wgpu::Texture) -> T, fallback: T) -> T {
        self.textures
            .get(texture.0, "texture")
            .map_or(fallback, read)
    }

    fn device_of(&self, kind: Kind, handle: u32) -> Option<u32> {
        self.owner(kind, handle)
    }

    fn limit_of(&self, limits: web::GpuSupportedLimits) -> wgpu::Limits {
        self.web
            .limits
            .get(limits.0, "supported limits")
            .cloned()
            .unwrap_or_default()
    }

    fn drop_object<T>(&mut self, kind: Kind, handle: u32, table: fn(&mut Self) -> &mut Table<T>) {
        table(self).remove(handle);
        self.forget(kind, handle);
    }
}

macro_rules! labels {
    ($($label:ident, $set:ident: $ty:ident => $kind:ident;)*) => {
        $(
            fn $label(&mut self, this: web::$ty) -> String {
                self.label_of(Kind::$kind, this.0)
            }

            fn $set(&mut self, this: web::$ty, label: &str) {
                self.relabel(Kind::$kind, this.0, label);
            }
        )*
    };
}

macro_rules! limits {
    ($($method:ident => $field:ident: $ty:ty,)*) => {
        $(
            fn $method(&mut self, this: web::GpuSupportedLimits) -> $ty {
                self.limit_of(this).$field
            }
        )*
    };
}

macro_rules! records {
    ($ty:ident, $value:ty, $table:ident, $new:ident, $add:ident, $get:ident, $has:ident, $remove:ident, $keys:ident, $values:ident, $entries:ident, $drop:ident) => {
        fn $new(&mut self) -> web::$ty {
            web::$ty(self.web.$table.insert(BTreeMap::new()))
        }

        fn $add(&mut self, this: web::$ty, key: &str, value: $value) {
            if let Ok(record) = self.web.$table.get_mut(this.0, "record") {
                record.insert(key.to_owned(), value);
            }
        }

        fn $get(&mut self, this: web::$ty, key: &str) -> Option<$value> {
            self.web.$table.get(this.0, "record").ok()?.get(key).copied()
        }

        fn $has(&mut self, this: web::$ty, key: &str) -> bool {
            self.web
                .$table
                .get(this.0, "record")
                .is_ok_and(|record| record.contains_key(key))
        }

        fn $remove(&mut self, this: web::$ty, key: &str) {
            if let Ok(record) = self.web.$table.get_mut(this.0, "record") {
                record.remove(key);
            }
        }

        fn $keys(&mut self, this: web::$ty) -> Vec<String> {
            self.web
                .$table
                .get(this.0, "record")
                .map(|record| record.keys().cloned().collect())
                .unwrap_or_default()
        }

        fn $values(&mut self, this: web::$ty) -> Vec<$value> {
            self.web
                .$table
                .get(this.0, "record")
                .map(|record| record.values().copied().collect())
                .unwrap_or_default()
        }

        fn $entries(&mut self, this: web::$ty) -> Vec<(String, $value)> {
            self.web
                .$table
                .get(this.0, "record")
                .map(|record| record.iter().map(|(key, value)| (key.clone(), *value)).collect())
                .unwrap_or_default()
        }

        fn $drop(&mut self, this: web::$ty) {
            self.web.$table.remove(this.0);
        }
    };
}

impl Host for Gpu {
    fn get_gpu(&mut self) -> web::Gpu {
        web::Gpu(self.web.gpus.insert(()))
    }

    fn gpu_request_adapter(
        &mut self,
        _this: web::Gpu,
        _options: Option<web::GpuRequestAdapterOptions>,
        settle: Promise<Option<web::GpuAdapter>>,
    ) {
        // The host's one adapter, whatever was asked: the power preference and a fallback are
        // hints, and every device it makes is core, which a compatibility request may get.
        let adapter = web::GpuAdapter(self.web.adapters.insert(()));
        self.web.settled.push(settle.settle(&Some(adapter)));
        self.settling.wake();
    }

    fn gpu_get_preferred_canvas_format(&mut self, _this: web::Gpu) -> web::GpuTextureFormat {
        web::GpuTextureFormat::Rgba8unorm
    }

    fn gpu_wgsl_language_features(&mut self, _this: web::Gpu) -> web::WgslLanguageFeatures {
        web::WgslLanguageFeatures(self.web.wgsl.insert(()))
    }

    /// The WGSL language extensions naga implements, which every host's shaders compile with.
    fn wgsl_language_features_has(&mut self, _this: web::WgslLanguageFeatures, value: &str) -> bool {
        [
            "readonly_and_readwrite_storage_textures",
            "packed_4x8_integer_dot_product",
            "pointer_composite_access",
        ]
        .contains(&value)
    }

    fn gpu_adapter_features(&mut self, _this: web::GpuAdapter) -> web::GpuSupportedFeatures {
        let mut names = convert::features(self.device.features());
        names.insert(0, web::GpuFeatureName::CoreFeaturesAndLimits);
        web::GpuSupportedFeatures(self.web.features.insert(names))
    }

    fn gpu_adapter_limits(&mut self, _this: web::GpuAdapter) -> web::GpuSupportedLimits {
        web::GpuSupportedLimits(self.web.limits.insert(self.device.limits()))
    }

    fn gpu_adapter_info(&mut self, _this: web::GpuAdapter) -> web::GpuAdapterInfo {
        web::GpuAdapterInfo(self.web.infos.insert(()))
    }

    fn gpu_adapter_request_device(
        &mut self,
        _this: web::GpuAdapter,
        descriptor: Option<web::GpuDeviceDescriptor>,
        settle: Promise<Result<web::GpuDevice, web::RequestDeviceError>>,
    ) {
        let device = self.new_device(descriptor);
        self.web.settled.push(settle.settle(&device));
        self.settling.wake();
    }

    // Adapter information is what browsers give a page that did not ask for more: nothing that
    // names the hardware.
    fn gpu_adapter_info_vendor(&mut self, _this: web::GpuAdapterInfo) -> String {
        String::new()
    }

    fn gpu_adapter_info_architecture(&mut self, _this: web::GpuAdapterInfo) -> String {
        String::new()
    }

    fn gpu_adapter_info_device(&mut self, _this: web::GpuAdapterInfo) -> String {
        String::new()
    }

    fn gpu_adapter_info_description(&mut self, _this: web::GpuAdapterInfo) -> String {
        String::new()
    }

    fn gpu_adapter_info_subgroup_min_size(&mut self, _this: web::GpuAdapterInfo) -> u32 {
        self.device.adapter_info().subgroup_min_size
    }

    fn gpu_adapter_info_subgroup_max_size(&mut self, _this: web::GpuAdapterInfo) -> u32 {
        self.device.adapter_info().subgroup_max_size
    }

    fn gpu_adapter_info_is_fallback_adapter(&mut self, _this: web::GpuAdapterInfo) -> bool {
        self.device.adapter_info().device_type == wgpu::DeviceType::Cpu
    }

    fn gpu_supported_features_has(&mut self, this: web::GpuSupportedFeatures, value: &str) -> bool {
        self.web.features.get(this.0, "supported features").is_ok_and(|names| {
            names
                .iter()
                .any(|name| convert::feature_name(*name) == value)
        })
    }

    limits! {
        gpu_supported_limits_max_texture_dimension1_d => max_texture_dimension_1d: u32,
        gpu_supported_limits_max_texture_dimension2_d => max_texture_dimension_2d: u32,
        gpu_supported_limits_max_texture_dimension3_d => max_texture_dimension_3d: u32,
        gpu_supported_limits_max_texture_array_layers => max_texture_array_layers: u32,
        gpu_supported_limits_max_bind_groups => max_bind_groups: u32,
        gpu_supported_limits_max_bind_groups_plus_vertex_buffers => max_bind_groups_plus_vertex_buffers: u32,
        gpu_supported_limits_max_immediate_size => max_immediate_size: u32,
        gpu_supported_limits_max_bindings_per_bind_group => max_bindings_per_bind_group: u32,
        gpu_supported_limits_max_dynamic_uniform_buffers_per_pipeline_layout => max_dynamic_uniform_buffers_per_pipeline_layout: u32,
        gpu_supported_limits_max_dynamic_storage_buffers_per_pipeline_layout => max_dynamic_storage_buffers_per_pipeline_layout: u32,
        gpu_supported_limits_max_sampled_textures_per_shader_stage => max_sampled_textures_per_shader_stage: u32,
        gpu_supported_limits_max_samplers_per_shader_stage => max_samplers_per_shader_stage: u32,
        gpu_supported_limits_max_storage_buffers_per_shader_stage => max_storage_buffers_per_shader_stage: u32,
        gpu_supported_limits_max_storage_buffers_in_vertex_stage => max_storage_buffers_per_shader_stage: u32,
        gpu_supported_limits_max_storage_buffers_in_fragment_stage => max_storage_buffers_per_shader_stage: u32,
        gpu_supported_limits_max_storage_textures_per_shader_stage => max_storage_textures_per_shader_stage: u32,
        gpu_supported_limits_max_storage_textures_in_vertex_stage => max_storage_textures_per_shader_stage: u32,
        gpu_supported_limits_max_storage_textures_in_fragment_stage => max_storage_textures_per_shader_stage: u32,
        gpu_supported_limits_max_uniform_buffers_per_shader_stage => max_uniform_buffers_per_shader_stage: u32,
        gpu_supported_limits_max_uniform_buffer_binding_size => max_uniform_buffer_binding_size: u64,
        gpu_supported_limits_max_storage_buffer_binding_size => max_storage_buffer_binding_size: u64,
        gpu_supported_limits_min_uniform_buffer_offset_alignment => min_uniform_buffer_offset_alignment: u32,
        gpu_supported_limits_min_storage_buffer_offset_alignment => min_storage_buffer_offset_alignment: u32,
        gpu_supported_limits_max_vertex_buffers => max_vertex_buffers: u32,
        gpu_supported_limits_max_buffer_size => max_buffer_size: u64,
        gpu_supported_limits_max_vertex_attributes => max_vertex_attributes: u32,
        gpu_supported_limits_max_vertex_buffer_array_stride => max_vertex_buffer_array_stride: u32,
        gpu_supported_limits_max_inter_stage_shader_variables => max_inter_stage_shader_variables: u32,
        gpu_supported_limits_max_color_attachments => max_color_attachments: u32,
        gpu_supported_limits_max_color_attachment_bytes_per_sample => max_color_attachment_bytes_per_sample: u32,
        gpu_supported_limits_max_compute_workgroup_storage_size => max_compute_workgroup_storage_size: u32,
        gpu_supported_limits_max_compute_invocations_per_workgroup => max_compute_invocations_per_workgroup: u32,
        gpu_supported_limits_max_compute_workgroup_size_x => max_compute_workgroup_size_x: u32,
        gpu_supported_limits_max_compute_workgroup_size_y => max_compute_workgroup_size_y: u32,
        gpu_supported_limits_max_compute_workgroup_size_z => max_compute_workgroup_size_z: u32,
        gpu_supported_limits_max_compute_workgroups_per_dimension => max_compute_workgroups_per_dimension: u32,
    }

    fn gpu_device_features(&mut self, this: web::GpuDevice) -> web::GpuSupportedFeatures {
        let names = self
            .web
            .devices
            .get(this.0, "device")
            .map(|device| device.names.clone())
            .unwrap_or_default();
        web::GpuSupportedFeatures(self.web.features.insert(names))
    }

    fn gpu_device_limits(&mut self, this: web::GpuDevice) -> web::GpuSupportedLimits {
        let limits = self
            .device_has(this.0)
            .map(|(_, limits)| limits)
            .unwrap_or_default();
        web::GpuSupportedLimits(self.web.limits.insert(limits))
    }

    fn gpu_device_adapter_info(&mut self, _this: web::GpuDevice) -> web::GpuAdapterInfo {
        web::GpuAdapterInfo(self.web.infos.insert(()))
    }

    fn gpu_device_queue(&mut self, this: web::GpuDevice) -> web::GpuQueue {
        let queue = self
            .web
            .devices
            .get(this.0, "device")
            .map_or(0, |device| device.queue);
        web::GpuQueue(queue)
    }

    fn gpu_device_destroy(&mut self, this: web::GpuDevice) {
        self.destroy_web_device(this.0);
    }

    fn gpu_device_create_buffer(&mut self, this: web::GpuDevice, descriptor: web::GpuBufferDescriptor) -> web::GpuBuffer {
        let mapped = descriptor.mapped_at_creation.unwrap_or(false);
        let limits = self.device_has(this.0).map(|(_, limits)| limits);
        let handle = self.make(Some(this.0), Kind::Buffer, descriptor.label.as_deref(), |gpu| &mut gpu.buffers, |gpu| {
            if let Some(limits) = &limits
                && descriptor.size > limits.max_buffer_size
            {
                return Err(Failure::validation(format!(
                    "a buffer of {} bytes is larger than the device's maxBufferSize, {}",
                    descriptor.size, limits.max_buffer_size
                )));
            }
            let usage: wgpu::BufferUsages = descriptor.usage.convert();
            Ok(gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: descriptor.label.as_deref(),
                size: descriptor.size,
                usage: gpu.resolvable(usage),
                mapped_at_creation: mapped,
            }))
        });
        if mapped && self.buffers.get(handle, "buffer").is_ok() {
            self.settling.mapped_at_creation(handle, descriptor.size);
        }
        self.web.buffers.insert(handle, descriptor.usage);
        web::GpuBuffer(handle)
    }

    fn gpu_device_create_texture(&mut self, this: web::GpuDevice, descriptor: web::GpuTextureDescriptor) -> web::GpuTexture {
        let allowed = self.allowed(this.0);
        let limits = self.device_has(this.0).map(|(_, limits)| limits);
        let handle = self.make(Some(this.0), Kind::Texture, descriptor.label.as_deref(), |gpu| &mut gpu.textures, |gpu| {
            let view_formats: Vec<wgpu::TextureFormat> = descriptor
                .view_formats
                .iter()
                .flatten()
                .map(|format| format.convert())
                .collect();
            let wgpu_descriptor = wgpu::TextureDescriptor {
                label: descriptor.label.as_deref(),
                size: descriptor.size.convert(),
                mip_level_count: descriptor.mip_level_count.unwrap_or(1),
                sample_count: descriptor.sample_count.unwrap_or(1),
                dimension: descriptor
                    .dimension
                    .map_or(wgpu::TextureDimension::D2, Convert::convert),
                format: descriptor.format.convert(),
                usage: descriptor.usage.convert(),
                view_formats: &view_formats,
            };
            allowed.texture(&wgpu_descriptor).map_err(Failure::validation)?;
            if let Some(limits) = &limits {
                let size = wgpu_descriptor.size;
                let largest = match wgpu_descriptor.dimension {
                    wgpu::TextureDimension::D1 => limits.max_texture_dimension_1d,
                    wgpu::TextureDimension::D2 => limits.max_texture_dimension_2d,
                    wgpu::TextureDimension::D3 => limits.max_texture_dimension_3d,
                };
                if size.width > largest || size.height > largest {
                    return Err(Failure::validation(format!(
                        "a {} by {} texture is larger than the device's limit, {largest}",
                        size.width, size.height
                    )));
                }
            }
            Ok(gpu.device.create_texture(&wgpu_descriptor))
        });
        self.made_textures.insert(handle);
        web::GpuTexture(handle)
    }

    fn gpu_device_create_sampler(&mut self, this: web::GpuDevice, descriptor: Option<web::GpuSamplerDescriptor>) -> web::GpuSampler {
        let descriptor = descriptor.unwrap_or(web::GpuSamplerDescriptor {
            address_mode_u: None,
            address_mode_v: None,
            address_mode_w: None,
            mag_filter: None,
            min_filter: None,
            mipmap_filter: None,
            lod_min_clamp: None,
            lod_max_clamp: None,
            compare: None,
            max_anisotropy: None,
            label: None,
        });
        let address = |mode: Option<web::GpuAddressMode>| mode.map_or(wgpu::AddressMode::ClampToEdge, Convert::convert);
        let handle = self.make(Some(this.0), Kind::Sampler, descriptor.label.as_deref(), |gpu| &mut gpu.samplers, |gpu| {
            Ok(gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: descriptor.label.as_deref(),
                address_mode_u: address(descriptor.address_mode_u),
                address_mode_v: address(descriptor.address_mode_v),
                address_mode_w: address(descriptor.address_mode_w),
                mag_filter: descriptor.mag_filter.map_or(wgpu::FilterMode::Nearest, Convert::convert),
                min_filter: descriptor.min_filter.map_or(wgpu::FilterMode::Nearest, Convert::convert),
                mipmap_filter: descriptor
                    .mipmap_filter
                    .map_or(wgpu::MipmapFilterMode::Nearest, Convert::convert),
                lod_min_clamp: descriptor.lod_min_clamp.unwrap_or(0.0),
                lod_max_clamp: descriptor.lod_max_clamp.unwrap_or(32.0),
                compare: descriptor.compare.map(Convert::convert),
                anisotropy_clamp: descriptor.max_anisotropy.unwrap_or(1),
                border_color: None,
            }))
        });
        web::GpuSampler(handle)
    }

    fn gpu_device_create_bind_group_layout(
        &mut self,
        this: web::GpuDevice,
        descriptor: web::GpuBindGroupLayoutDescriptor,
    ) -> web::GpuBindGroupLayout {
        let allowed = self.allowed(this.0);
        let handle = self.make(Some(this.0), Kind::GroupLayout, descriptor.label.as_deref(), |gpu| &mut gpu.group_layouts, |gpu| {
            let mut entries = Vec::with_capacity(descriptor.entries.len());
            for entry in &descriptor.entries {
                if convert::binding_kinds(entry) != 1 {
                    return Err(Failure::validation(format!(
                        "bind group layout entry {} names {} binding kinds, not exactly one",
                        entry.binding,
                        convert::binding_kinds(entry)
                    )));
                }
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding: entry.binding,
                    visibility: entry.visibility.convert(),
                    ty: entry.convert(),
                    count: None,
                });
            }
            let wgpu_descriptor = wgpu::BindGroupLayoutDescriptor {
                label: descriptor.label.as_deref(),
                entries: &entries,
            };
            allowed.group_layout(&wgpu_descriptor).map_err(Failure::validation)?;
            Ok(gpu.device.create_bind_group_layout(&wgpu_descriptor))
        });
        web::GpuBindGroupLayout(handle)
    }

    fn gpu_device_create_pipeline_layout(
        &mut self,
        this: web::GpuDevice,
        descriptor: web::GpuPipelineLayoutDescriptor,
    ) -> web::GpuPipelineLayout {
        let handle = self.make(Some(this.0), Kind::PipelineLayout, descriptor.label.as_deref(), |gpu| &mut gpu.pipeline_layouts, |gpu| {
            let layouts = descriptor
                .bind_group_layouts
                .iter()
                .map(|layout| {
                    layout
                        .map(|layout| found(gpu.group_layouts.get(layout.0, "bind group layout")).cloned())
                        .transpose()
                })
                .collect::<Result<Vec<_>, _>>()?;
            let borrowed: Vec<_> = layouts.iter().map(Option::as_ref).collect();
            Ok(gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: descriptor.label.as_deref(),
                bind_group_layouts: &borrowed,
                immediate_size: descriptor.immediate_size.unwrap_or(0),
            }))
        });
        web::GpuPipelineLayout(handle)
    }

    fn gpu_device_create_bind_group(&mut self, this: web::GpuDevice, descriptor: web::GpuBindGroupDescriptor) -> web::GpuBindGroup {
        let handle = self.make(Some(this.0), Kind::Group, descriptor.label.as_deref(), |gpu| &mut gpu.groups, |gpu| {
            gpu.try_bind_group(&descriptor)
        });
        web::GpuBindGroup(handle)
    }

    fn gpu_device_create_shader_module(
        &mut self,
        this: web::GpuDevice,
        descriptor: web::GpuShaderModuleDescriptor,
    ) -> web::GpuShaderModule {
        let allowed = self.allowed(this.0);
        // A module whose WGSL is rejected is still made: its compilation info says why.
        let handle = self.make(Some(this.0), Kind::Module, descriptor.label.as_deref(), |gpu| &mut gpu.modules, |gpu| {
            if let Err(message) = allowed.shader(&descriptor.code) {
                gpu.raise(Some(this.0), Failure::validation(message));
            }
            Ok(gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: descriptor.label.as_deref(),
                source: wgpu::ShaderSource::Wgsl(descriptor.code.as_str().into()),
            }))
        });
        web::GpuShaderModule(handle)
    }

    fn gpu_device_create_compute_pipeline(
        &mut self,
        this: web::GpuDevice,
        descriptor: web::GpuComputePipelineDescriptor,
    ) -> web::GpuComputePipeline {
        let handle = self.make(Some(this.0), Kind::ComputePipeline, descriptor.label.as_deref(), |gpu| &mut gpu.compute_pipelines, |gpu| {
            gpu.try_compute_pipeline(&descriptor)
        });
        web::GpuComputePipeline(handle)
    }

    fn gpu_device_create_render_pipeline(
        &mut self,
        this: web::GpuDevice,
        descriptor: web::GpuRenderPipelineDescriptor,
    ) -> web::GpuRenderPipeline {
        let handle = self.make(Some(this.0), Kind::RenderPipeline, descriptor.label.as_deref(), |gpu| &mut gpu.pipelines, |gpu| {
            gpu.try_render_pipeline(this.0, &descriptor)
        });
        web::GpuRenderPipeline(handle)
    }

    fn gpu_device_create_compute_pipeline_async(
        &mut self,
        this: web::GpuDevice,
        descriptor: web::GpuComputePipelineDescriptor,
        settle: Promise<Result<web::GpuComputePipeline, web::CreatePipelineError>>,
    ) {
        let answer = self.made_async(this.0, Kind::ComputePipeline, descriptor.label.clone(), |gpu| &mut gpu.compute_pipelines, |gpu| {
            gpu.try_compute_pipeline(&descriptor)
        });
        self.web.settled.push(settle.settle(&answer.map(web::GpuComputePipeline)));
        self.settling.wake();
    }

    fn gpu_device_create_render_pipeline_async(
        &mut self,
        this: web::GpuDevice,
        descriptor: web::GpuRenderPipelineDescriptor,
        settle: Promise<Result<web::GpuRenderPipeline, web::CreatePipelineError>>,
    ) {
        let answer = self.made_async(this.0, Kind::RenderPipeline, descriptor.label.clone(), |gpu| &mut gpu.pipelines, |gpu| {
            gpu.try_render_pipeline(this.0, &descriptor)
        });
        self.web.settled.push(settle.settle(&answer.map(web::GpuRenderPipeline)));
        self.settling.wake();
    }

    fn gpu_device_create_command_encoder(
        &mut self,
        this: web::GpuDevice,
        descriptor: Option<web::GpuCommandEncoderDescriptor>,
    ) -> web::GpuCommandEncoder {
        let label = descriptor.and_then(|descriptor| descriptor.label);
        let handle = self.make(Some(this.0), Kind::Encoder, label.as_deref(), |gpu| &mut gpu.encoders, |gpu| {
            Ok(gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: label.as_deref(),
            }))
        });
        web::GpuCommandEncoder(handle)
    }

    fn gpu_device_create_render_bundle_encoder(
        &mut self,
        this: web::GpuDevice,
        descriptor: web::GpuRenderBundleEncoderDescriptor,
    ) -> web::GpuRenderBundleEncoder {
        web::GpuRenderBundleEncoder(self.bundle_encoder(this.0, descriptor))
    }

    fn gpu_device_create_query_set(
        &mut self,
        this: web::GpuDevice,
        descriptor: web::GpuQuerySetDescriptor,
    ) -> Result<web::GpuQuerySet, web::CreateQuerySetError> {
        let allowed = self.allowed(this.0);
        let kind: wgpu::QueryType = descriptor.r#type.convert();
        if allowed.query_set(kind).is_err() {
            return Err(web::CreateQuerySetError {
                kind: web::CreateQuerySetErrorKind::TypeError,
                message: "timestamp queries need the timestamp-query feature".to_owned(),
            });
        }
        let handle = self.make(Some(this.0), Kind::QuerySet, descriptor.label.as_deref(), |gpu| &mut gpu.query_sets, |gpu| {
            Ok(QuerySet {
                set: gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: descriptor.label.as_deref(),
                    ty: kind,
                    count: descriptor.count,
                }),
                timestamps: matches!(kind, wgpu::QueryType::Timestamp),
            })
        });
        self.web.query_sets.insert(handle, (descriptor.r#type, descriptor.count));
        Ok(web::GpuQuerySet(handle))
    }

    fn gpu_device_lost(&mut self, this: web::GpuDevice, settle: Promise<web::GpuDeviceLostInfo>) {
        if let Ok(device) = self.web.devices.get_mut(this.0, "device") {
            device.losing.push(settle);
        }
        self.settling.wake();
    }

    fn gpu_device_push_error_scope(&mut self, this: web::GpuDevice, filter: web::GpuErrorFilter) {
        self.push_scope(this.0, filter);
    }

    fn gpu_device_pop_error_scope(
        &mut self,
        this: web::GpuDevice,
        settle: Promise<Result<Option<web::GpuError>, web::PopErrorScopeError>>,
    ) {
        self.pop_scope(this.0, settle);
        self.settling.wake();
    }

    fn gpu_device_on_uncaptured_error(&mut self, this: web::GpuDevice, settle: Sender<web::GpuError>) {
        if let Ok(device) = self.web.devices.get_mut(this.0, "device") {
            device.uncaptured.push(settle);
        }
    }

    fn gpu_device_lost_info_reason(&mut self, this: web::GpuDeviceLostInfo) -> web::GpuDeviceLostReason {
        self.web
            .lost_infos
            .get(this.0, "device lost info")
            .map_or(web::GpuDeviceLostReason::Unknown, |(reason, _)| *reason)
    }

    fn gpu_device_lost_info_message(&mut self, this: web::GpuDeviceLostInfo) -> String {
        self.web
            .lost_infos
            .get(this.0, "device lost info")
            .map(|(_, message)| message.clone())
            .unwrap_or_default()
    }

    fn gpu_error_message(&mut self, this: web::GpuError) -> String {
        self.web
            .errors
            .get(this.0, "error")
            .map(|failure| failure.message.clone())
            .unwrap_or_default()
    }

    fn gpu_error_kind(&mut self, this: web::GpuError) -> web::GpuErrorKind {
        match self.web.errors.get(this.0, "error").map(|failure| failure.kind) {
            Ok(super::ErrorKind::OutOfMemory) => web::GpuErrorKind::OutOfMemoryError,
            Ok(super::ErrorKind::Internal) => web::GpuErrorKind::InternalError,
            _ => web::GpuErrorKind::ValidationError,
        }
    }

    fn gpu_uncaptured_error_event_error(&mut self, this: web::GpuUncapturedErrorEvent) -> web::GpuError {
        let error = self.web.events.get(this.0, "uncaptured error event").copied().unwrap_or(0);
        web::GpuError(error)
    }

    fn gpu_buffer_size(&mut self, this: web::GpuBuffer) -> u64 {
        self.buffers.get(this.0, "buffer").map_or(0, wgpu::Buffer::size)
    }

    fn gpu_buffer_usage(&mut self, this: web::GpuBuffer) -> web::GpuBufferUsage {
        self.web
            .buffers
            .get(&this.0)
            .copied()
            .unwrap_or(web::GpuBufferUsage::empty())
    }

    fn gpu_buffer_map_state(&mut self, this: web::GpuBuffer) -> web::GpuBufferMapState {
        match self.settling.map_state(this.0) {
            None => web::GpuBufferMapState::Unmapped,
            Some(false) => web::GpuBufferMapState::Pending,
            Some(true) => web::GpuBufferMapState::Mapped,
        }
    }

    fn gpu_buffer_map_async(
        &mut self,
        this: web::GpuBuffer,
        mode: web::GpuMapMode,
        offset: Option<u64>,
        size: Option<u64>,
        settle: Promise<Result<(), web::MapAsyncError>>,
    ) {
        let device = self.device_of(Kind::Buffer, this.0);
        let refuse = |kind, message: String| Err(web::MapAsyncError { kind, message });
        if device.is_some_and(|device| self.device_lost(device)) {
            let answer = refuse(web::MapAsyncErrorKind::OperationError, "the buffer's device is lost".to_owned());
            self.web.settled.push(settle.settle(&answer));
            return self.settling.wake();
        }
        let mapping = match (mode.contains(web::GpuMapMode::READ), mode.contains(web::GpuMapMode::WRITE)) {
            (true, false) => Ok(abi::MapMode::Read),
            (false, true) => Ok(abi::MapMode::Write),
            _ => Err(Failure::validation("a map is for reading or for writing, not both or neither")),
        };
        let started = mapping.and_then(|mapping| {
            let (buffer, offset, size) = self.buffer_range(this.0, offset, size)?;
            let end = offset
                .checked_add(size)
                .filter(|end| *end <= buffer.size())
                .ok_or_else(|| Failure::validation(format!("a map of {size} bytes from {offset} runs past the end of its buffer")))?;
            self.settling
                .map(&self.device, &buffer, this.0, mapping, offset..end)
                .map_err(Failure::validation)
        });
        match started {
            Ok(ticket) => {
                self.web.maps.insert(ticket, (this.0, settle));
            }
            Err(failure) => {
                let message = failure.message.clone();
                self.raise(device, failure);
                self.web
                    .settled
                    .push(settle.settle(&refuse(web::MapAsyncErrorKind::OperationError, message)));
                self.settling.wake();
            }
        }
    }

    fn gpu_buffer_get_mapped_range_get_with_copy(
        &mut self,
        this: web::GpuBuffer,
        offset: Option<u64>,
        size: Option<u64>,
    ) -> Result<Vec<u8>, web::GetMappedRangeError> {
        let refuse = |message: String| web::GetMappedRangeError {
            kind: web::GetMappedRangeErrorKind::OperationError,
            message,
        };
        let (_, offset, size) = self.buffer_range(this.0, offset, size).map_err(|failure| refuse(failure.message))?;
        let (buffer, range) = self.mapped_range(abi::Buffer(this.0), offset, size, false).map_err(refuse)?;
        match range.is_empty() {
            true => Ok(Vec::new()),
            false => buffer
                .slice(range)
                .get_mapped_range()
                .map(|mapped| mapped.to_vec())
                .map_err(|error| refuse(error.to_string())),
        }
    }

    fn gpu_buffer_get_mapped_range_set_with_copy(
        &mut self,
        this: web::GpuBuffer,
        data: &[u8],
        offset: Option<u64>,
        size: Option<u64>,
    ) -> Result<(), web::GetMappedRangeError> {
        let refuse = |message: String| web::GetMappedRangeError {
            kind: web::GetMappedRangeErrorKind::OperationError,
            message,
        };
        let size = size.unwrap_or(data.len() as u64);
        if size != data.len() as u64 {
            return Err(web::GetMappedRangeError {
                kind: web::GetMappedRangeErrorKind::RangeError,
                message: format!("{} bytes were given for a range of {size}", data.len()),
            });
        }
        let offset = offset.unwrap_or(0);
        let (buffer, range) = self.mapped_range(abi::Buffer(this.0), offset, size, true).map_err(refuse)?;
        if !range.is_empty() {
            buffer
                .slice(range)
                .get_mapped_range_mut()
                .map_err(|error| refuse(error.to_string()))?
                .copy_from_slice(data);
        }
        Ok(())
    }

    fn gpu_buffer_unmap(&mut self, this: web::GpuBuffer) -> Result<(), web::UnmapError> {
        self.settling.forget(this.0);
        if let Ok(buffer) = self.buffers.get(this.0, "buffer") {
            buffer.unmap();
        }
        Ok(())
    }

    fn gpu_buffer_destroy(&mut self, this: web::GpuBuffer) {
        self.settling.forget(this.0);
        if let Ok(buffer) = self.buffers.get(this.0, "buffer") {
            buffer.destroy();
        }
    }

    fn gpu_texture_create_view(
        &mut self,
        this: web::GpuTexture,
        descriptor: Option<web::GpuTextureViewDescriptor>,
    ) -> web::GpuTextureView {
        let device = self.device_of(Kind::Texture, this.0);
        let label = descriptor.as_ref().and_then(|descriptor| descriptor.label.clone());
        let handle = self.make(device, Kind::View, label.as_deref(), |gpu| &mut gpu.views, |gpu| {
            let texture = found(gpu.textures.get(this.0, "texture"))?;
            let Some(descriptor) = descriptor else {
                return Ok(texture.create_view(&wgpu::TextureViewDescriptor::default()));
            };
            if descriptor.swizzle.as_deref().is_some_and(|swizzle| swizzle != "rgba") {
                return Err(Failure::validation(
                    "a swizzled view needs the texture-component-swizzle feature",
                ));
            }
            Ok(texture.create_view(&wgpu::TextureViewDescriptor {
                label: descriptor.label.as_deref(),
                format: descriptor.format.map(Convert::convert),
                dimension: descriptor.dimension.map(Convert::convert),
                usage: descriptor.usage.map(Convert::convert),
                aspect: descriptor.aspect.map_or(wgpu::TextureAspect::All, Convert::convert),
                base_mip_level: descriptor.base_mip_level.unwrap_or(0),
                mip_level_count: descriptor.mip_level_count,
                base_array_layer: descriptor.base_array_layer.unwrap_or(0),
                array_layer_count: descriptor.array_layer_count,
            }))
        });
        web::GpuTextureView(handle)
    }

    fn gpu_texture_destroy(&mut self, this: web::GpuTexture) {
        if !self.made_textures.contains(&this.0) {
            return;
        }
        if let Ok(texture) = self.textures.get(this.0, "texture") {
            texture.destroy();
        }
    }

    fn gpu_texture_width(&mut self, this: web::GpuTexture) -> u32 {
        self.texture_of(this, wgpu::Texture::width, 0)
    }

    fn gpu_texture_height(&mut self, this: web::GpuTexture) -> u32 {
        self.texture_of(this, wgpu::Texture::height, 0)
    }

    fn gpu_texture_depth_or_array_layers(&mut self, this: web::GpuTexture) -> u32 {
        self.texture_of(this, wgpu::Texture::depth_or_array_layers, 0)
    }

    fn gpu_texture_mip_level_count(&mut self, this: web::GpuTexture) -> u32 {
        self.texture_of(this, wgpu::Texture::mip_level_count, 0)
    }

    fn gpu_texture_sample_count(&mut self, this: web::GpuTexture) -> u32 {
        self.texture_of(this, wgpu::Texture::sample_count, 0)
    }

    fn gpu_texture_dimension(&mut self, this: web::GpuTexture) -> web::GpuTextureDimension {
        self.texture_of(
            this,
            |texture| TryConvert::try_convert(texture.dimension()).unwrap_or(web::GpuTextureDimension::D2),
            web::GpuTextureDimension::D2,
        )
    }

    fn gpu_texture_format(&mut self, this: web::GpuTexture) -> web::GpuTextureFormat {
        self.texture_of(
            this,
            |texture| TryConvert::try_convert(texture.format()).unwrap_or(web::GpuTextureFormat::Rgba8unorm),
            web::GpuTextureFormat::Rgba8unorm,
        )
    }

    fn gpu_texture_usage(&mut self, this: web::GpuTexture) -> web::GpuTextureUsage {
        self.texture_of(this, |texture| texture.usage().convert(), web::GpuTextureUsage::empty())
    }

    fn gpu_texture_texture_binding_view_dimension(&mut self, _this: web::GpuTexture) -> Option<web::GpuTextureViewDimension> {
        None
    }

    fn gpu_shader_module_get_compilation_info(&mut self, this: web::GpuShaderModule, settle: Promise<web::GpuCompilationInfo>) {
        match self.modules.get(this.0, "shader module") {
            Ok(module) => {
                let compiling: super::Compiling = Box::pin(module.get_compilation_info());
                self.web.compiling.push((compiling, settle));
            }
            Err(_) => {
                let info = web::GpuCompilationInfo(self.web.compilation_infos.insert(Vec::new()));
                self.web.settled.push(settle.settle(&info));
            }
        }
        self.settling.wake();
    }

    fn gpu_compilation_info_messages(&mut self, this: web::GpuCompilationInfo) -> Vec<web::GpuCompilationMessage> {
        self.web
            .compilation_infos
            .get(this.0, "compilation info")
            .map(|messages| messages.iter().map(|message| web::GpuCompilationMessage(*message)).collect())
            .unwrap_or_default()
    }

    fn gpu_compilation_message_message(&mut self, this: web::GpuCompilationMessage) -> String {
        self.web.messages.get(this.0, "message").map(|message| message.message.clone()).unwrap_or_default()
    }

    fn gpu_compilation_message_type(&mut self, this: web::GpuCompilationMessage) -> web::GpuCompilationMessageType {
        self.web
            .messages
            .get(this.0, "message")
            .map_or(web::GpuCompilationMessageType::Info, |message| message.kind)
    }

    fn gpu_compilation_message_line_num(&mut self, this: web::GpuCompilationMessage) -> u64 {
        self.web.messages.get(this.0, "message").map_or(0, |message| message.line)
    }

    fn gpu_compilation_message_line_pos(&mut self, this: web::GpuCompilationMessage) -> u64 {
        self.web.messages.get(this.0, "message").map_or(0, |message| message.position)
    }

    fn gpu_compilation_message_offset(&mut self, this: web::GpuCompilationMessage) -> u64 {
        self.web.messages.get(this.0, "message").map_or(0, |message| message.offset)
    }

    fn gpu_compilation_message_length(&mut self, this: web::GpuCompilationMessage) -> u64 {
        self.web.messages.get(this.0, "message").map_or(0, |message| message.length)
    }

    fn gpu_render_pipeline_get_bind_group_layout(&mut self, this: web::GpuRenderPipeline, index: u32) -> web::GpuBindGroupLayout {
        let device = self.device_of(Kind::RenderPipeline, this.0);
        let handle = self.make(device, Kind::GroupLayout, None, |gpu| &mut gpu.group_layouts, |gpu| {
            Ok(found(gpu.pipelines.get(this.0, "render pipeline"))?.get_bind_group_layout(index))
        });
        web::GpuBindGroupLayout(handle)
    }

    fn gpu_compute_pipeline_get_bind_group_layout(&mut self, this: web::GpuComputePipeline, index: u32) -> web::GpuBindGroupLayout {
        let device = self.device_of(Kind::ComputePipeline, this.0);
        let handle = self.make(device, Kind::GroupLayout, None, |gpu| &mut gpu.group_layouts, |gpu| {
            Ok(found(gpu.compute_pipelines.get(this.0, "compute pipeline"))?.get_bind_group_layout(index))
        });
        web::GpuBindGroupLayout(handle)
    }

    fn gpu_query_set_destroy(&mut self, this: web::GpuQuerySet) {
        if let Ok(set) = self.query_sets.get(this.0, "query set") {
            set.set.destroy();
        }
    }

    fn gpu_query_set_type(&mut self, this: web::GpuQuerySet) -> web::GpuQueryType {
        self.web
            .query_sets
            .get(&this.0)
            .map_or(web::GpuQueryType::Occlusion, |(kind, _)| *kind)
    }

    fn gpu_query_set_count(&mut self, this: web::GpuQuerySet) -> u32 {
        self.web.query_sets.get(&this.0).map_or(0, |(_, count)| *count)
    }

    records!(
        RecordGpuPipelineConstantValue, f64, constants,
        record_gpu_pipeline_constant_value_new,
        record_gpu_pipeline_constant_value_add,
        record_gpu_pipeline_constant_value_get,
        record_gpu_pipeline_constant_value_has,
        record_gpu_pipeline_constant_value_remove,
        record_gpu_pipeline_constant_value_keys,
        record_gpu_pipeline_constant_value_values,
        record_gpu_pipeline_constant_value_entries,
        record_gpu_pipeline_constant_value_drop
    );

    records!(
        RecordOptionGpuSize64, Option<u64>, sizes,
        record_option_gpu_size64_new,
        record_option_gpu_size64_add,
        record_option_gpu_size64_get,
        record_option_gpu_size64_has,
        record_option_gpu_size64_remove,
        record_option_gpu_size64_keys,
        record_option_gpu_size64_values,
        record_option_gpu_size64_entries,
        record_option_gpu_size64_drop
    );

    labels! {
        gpu_bind_group_label, gpu_bind_group_set_label: GpuBindGroup => Group;
        gpu_bind_group_layout_label, gpu_bind_group_layout_set_label: GpuBindGroupLayout => GroupLayout;
        gpu_buffer_label, gpu_buffer_set_label: GpuBuffer => Buffer;
        gpu_command_buffer_label, gpu_command_buffer_set_label: GpuCommandBuffer => CommandBuffer;
        gpu_command_encoder_label, gpu_command_encoder_set_label: GpuCommandEncoder => Encoder;
        gpu_compute_pass_encoder_label, gpu_compute_pass_encoder_set_label: GpuComputePassEncoder => ComputePass;
        gpu_compute_pipeline_label, gpu_compute_pipeline_set_label: GpuComputePipeline => ComputePipeline;
        gpu_device_label, gpu_device_set_label: GpuDevice => Device;
        gpu_pipeline_layout_label, gpu_pipeline_layout_set_label: GpuPipelineLayout => PipelineLayout;
        gpu_query_set_label, gpu_query_set_set_label: GpuQuerySet => QuerySet;
        gpu_queue_label, gpu_queue_set_label: GpuQueue => Queue;
        gpu_render_bundle_label, gpu_render_bundle_set_label: GpuRenderBundle => Bundle;
        gpu_render_bundle_encoder_label, gpu_render_bundle_encoder_set_label: GpuRenderBundleEncoder => BundleEncoder;
        gpu_render_pass_encoder_label, gpu_render_pass_encoder_set_label: GpuRenderPassEncoder => RenderPass;
        gpu_render_pipeline_label, gpu_render_pipeline_set_label: GpuRenderPipeline => RenderPipeline;
        gpu_sampler_label, gpu_sampler_set_label: GpuSampler => Sampler;
        gpu_shader_module_label, gpu_shader_module_set_label: GpuShaderModule => Module;
        gpu_texture_label, gpu_texture_set_label: GpuTexture => Texture;
        gpu_texture_view_label, gpu_texture_view_set_label: GpuTextureView => View;
    }

    // Encoders, passes, bundles and the queue (passes.rs).

    fn gpu_command_encoder_begin_render_pass(&mut self, this: web::GpuCommandEncoder, descriptor: web::GpuRenderPassDescriptor) -> web::GpuRenderPassEncoder {
        web::GpuRenderPassEncoder(self.begin_render_pass(this.0, descriptor))
    }

    fn gpu_command_encoder_begin_compute_pass(&mut self, this: web::GpuCommandEncoder, descriptor: Option<web::GpuComputePassDescriptor>) -> web::GpuComputePassEncoder {
        web::GpuComputePassEncoder(self.begin_compute_pass(this.0, descriptor))
    }

    fn gpu_command_encoder_copy_buffer_to_buffer(
        &mut self,
        this: web::GpuCommandEncoder,
        source: web::GpuBuffer,
        source_offset: Option<u64>,
        destination: web::GpuBuffer,
        destination_offset: Option<u64>,
        size: Option<u64>,
    ) {
        self.record(this.0, |gpu| {
            let from = found(gpu.buffers.get(source.0, "buffer"))?.clone();
            let to = found(gpu.buffers.get(destination.0, "buffer"))?.clone();
            let (source_offset, destination_offset) = (source_offset.unwrap_or(0), destination_offset.unwrap_or(0));
            Ok(Box::new(move |encoder: &mut wgpu::CommandEncoder| {
                encoder.copy_buffer_to_buffer(&from, source_offset, &to, destination_offset, size)
            }))
        });
    }

    fn gpu_command_encoder_copy_buffer_to_texture(
        &mut self,
        this: web::GpuCommandEncoder,
        source: web::GpuTexelCopyBufferInfo,
        destination: web::GpuTexelCopyTextureInfo,
        copy_size: web::GpuExtent3D,
    ) {
        self.record(this.0, |gpu| {
            let from = found(gpu.buffers.get(source.buffer.0, "buffer"))?.clone();
            let to = found(gpu.textures.get(destination.texture.0, "texture"))?.clone();
            Ok(Box::new(move |encoder: &mut wgpu::CommandEncoder| {
                encoder.copy_buffer_to_texture(buffer_info(&from, &source), texture_info(&to, &destination), copy_size.convert())
            }))
        });
    }

    fn gpu_command_encoder_copy_texture_to_buffer(
        &mut self,
        this: web::GpuCommandEncoder,
        source: web::GpuTexelCopyTextureInfo,
        destination: web::GpuTexelCopyBufferInfo,
        copy_size: web::GpuExtent3D,
    ) {
        self.record(this.0, |gpu| {
            let from = found(gpu.textures.get(source.texture.0, "texture"))?.clone();
            let to = found(gpu.buffers.get(destination.buffer.0, "buffer"))?.clone();
            Ok(Box::new(move |encoder: &mut wgpu::CommandEncoder| {
                encoder.copy_texture_to_buffer(texture_info(&from, &source), buffer_info(&to, &destination), copy_size.convert())
            }))
        });
    }

    fn gpu_command_encoder_copy_texture_to_texture(
        &mut self,
        this: web::GpuCommandEncoder,
        source: web::GpuTexelCopyTextureInfo,
        destination: web::GpuTexelCopyTextureInfo,
        copy_size: web::GpuExtent3D,
    ) {
        self.record(this.0, |gpu| {
            let from = found(gpu.textures.get(source.texture.0, "texture"))?.clone();
            let to = found(gpu.textures.get(destination.texture.0, "texture"))?.clone();
            Ok(Box::new(move |encoder: &mut wgpu::CommandEncoder| {
                encoder.copy_texture_to_texture(texture_info(&from, &source), texture_info(&to, &destination), copy_size.convert())
            }))
        });
    }

    fn gpu_command_encoder_clear_buffer(&mut self, this: web::GpuCommandEncoder, buffer: web::GpuBuffer, offset: Option<u64>, size: Option<u64>) {
        self.record(this.0, |gpu| {
            let buffer = found(gpu.buffers.get(buffer.0, "buffer"))?.clone();
            let offset = offset.unwrap_or(0);
            Ok(Box::new(move |encoder: &mut wgpu::CommandEncoder| encoder.clear_buffer(&buffer, offset, size)))
        });
    }

    fn gpu_command_encoder_resolve_query_set(
        &mut self,
        this: web::GpuCommandEncoder,
        query_set: web::GpuQuerySet,
        first_query: u32,
        query_count: u32,
        destination: web::GpuBuffer,
        destination_offset: u64,
    ) {
        self.resolve(this.0, query_set.0, first_query, query_count, destination.0, destination_offset);
    }

    fn gpu_command_encoder_finish(&mut self, this: web::GpuCommandEncoder, descriptor: Option<web::GpuCommandBufferDescriptor>) -> web::GpuCommandBuffer {
        let device = self.device_of(Kind::Encoder, this.0);
        let label = descriptor.and_then(|descriptor| descriptor.label);
        let handle = self.make(device, Kind::CommandBuffer, label.as_deref(), |gpu| &mut gpu.command_buffers, |gpu| {
            found(gpu.encoders.take(this.0, "command encoder")).map(wgpu::CommandEncoder::finish)
        });
        web::GpuCommandBuffer(handle)
    }

    fn gpu_command_encoder_push_debug_group(&mut self, this: web::GpuCommandEncoder, group_label: &str) {
        let label = group_label.to_owned();
        self.record(this.0, |_| Ok(Box::new(move |encoder: &mut wgpu::CommandEncoder| encoder.push_debug_group(&label))));
    }

    fn gpu_command_encoder_pop_debug_group(&mut self, this: web::GpuCommandEncoder) {
        self.record(this.0, |_| Ok(Box::new(|encoder: &mut wgpu::CommandEncoder| encoder.pop_debug_group())));
    }

    fn gpu_command_encoder_insert_debug_marker(&mut self, this: web::GpuCommandEncoder, marker_label: &str) {
        let label = marker_label.to_owned();
        self.record(this.0, |_| Ok(Box::new(move |encoder: &mut wgpu::CommandEncoder| encoder.insert_debug_marker(&label))));
    }

    fn gpu_compute_pass_encoder_set_pipeline(&mut self, this: web::GpuComputePassEncoder, pipeline: web::GpuComputePipeline) {
        self.compute(this.0, |gpu, pass| {
            pass.set_pipeline(found(gpu.compute_pipelines.get(pipeline.0, "compute pipeline"))?);
            Ok(())
        });
    }

    fn gpu_compute_pass_encoder_dispatch_workgroups(&mut self, this: web::GpuComputePassEncoder, x: u32, y: Option<u32>, z: Option<u32>) {
        self.compute(this.0, |_, pass| {
            pass.dispatch_workgroups(x, y.unwrap_or(1), z.unwrap_or(1));
            Ok(())
        });
    }

    fn gpu_compute_pass_encoder_dispatch_workgroups_indirect(&mut self, this: web::GpuComputePassEncoder, indirect_buffer: web::GpuBuffer, indirect_offset: u64) {
        self.compute(this.0, |gpu, pass| {
            let buffer = found(gpu.buffers.get(indirect_buffer.0, "buffer"))?.clone();
            pass.dispatch_workgroups_indirect(&buffer, indirect_offset);
            Ok(())
        });
    }

    fn gpu_compute_pass_encoder_end(&mut self, this: web::GpuComputePassEncoder) {
        let device = self.device_of(Kind::ComputePass, this.0);
        if device.is_some_and(|device| self.device_lost(device)) {
            return;
        }
        self.wrapped(device, |gpu| found(gpu.compute_passes.take(this.0, "compute pass")).map(drop));
    }

    fn gpu_compute_pass_encoder_push_debug_group(&mut self, this: web::GpuComputePassEncoder, group_label: &str) {
        self.compute(this.0, |_, pass| {
            pass.push_debug_group(group_label);
            Ok(())
        });
    }

    fn gpu_compute_pass_encoder_pop_debug_group(&mut self, this: web::GpuComputePassEncoder) {
        self.compute(this.0, |_, pass| {
            pass.pop_debug_group();
            Ok(())
        });
    }

    fn gpu_compute_pass_encoder_insert_debug_marker(&mut self, this: web::GpuComputePassEncoder, marker_label: &str) {
        self.compute(this.0, |_, pass| {
            pass.insert_debug_marker(marker_label);
            Ok(())
        });
    }

    fn gpu_compute_pass_encoder_set_bind_group(
        &mut self,
        this: web::GpuComputePassEncoder,
        index: u32,
        bind_group: Option<web::GpuBindGroup>,
        dynamic_offsets_data: Option<&[u32]>,
        dynamic_offsets_data_start: Option<u64>,
        dynamic_offsets_data_length: Option<u32>,
    ) -> Result<(), web::SetBindGroupError> {
        let offsets = offsets(dynamic_offsets_data, dynamic_offsets_data_start, dynamic_offsets_data_length)?;
        self.compute(this.0, |gpu, pass| {
            let group = bind_group
                .map(|group| found(gpu.groups.get(group.0, "bind group")).cloned())
                .transpose()?;
            pass.set_bind_group(index, group.as_ref(), offsets);
            Ok(())
        });
        Ok(())
    }

    fn gpu_compute_pass_encoder_set_immediates(
        &mut self,
        this: web::GpuComputePassEncoder,
        range_offset: u32,
        data: &[u8],
        data_offset: Option<u64>,
        data_size: Option<u64>,
    ) {
        match immediates(data, data_offset, data_size) {
            Ok(data) => self.compute(this.0, |_, pass| {
                pass.set_immediates(range_offset, data);
                Ok(())
            }),
            Err(failure) => {
                let device = self.device_of(Kind::ComputePass, this.0);
                self.raise(device, failure);
            }
        }
    }

    fn gpu_render_pass_encoder_set_viewport(&mut self, this: web::GpuRenderPassEncoder, x: f32, y: f32, width: f32, height: f32, min_depth: f32, max_depth: f32) {
        self.render(this.0, |_, pass| {
            pass.set_viewport(x, y, width, height, min_depth, max_depth);
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_set_scissor_rect(&mut self, this: web::GpuRenderPassEncoder, x: u32, y: u32, width: u32, height: u32) {
        self.render(this.0, |_, pass| {
            pass.set_scissor_rect(x, y, width, height);
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_set_blend_constant(&mut self, this: web::GpuRenderPassEncoder, color: web::GpuColor) {
        self.render(this.0, |_, pass| {
            pass.set_blend_constant(color.convert());
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_set_stencil_reference(&mut self, this: web::GpuRenderPassEncoder, reference: u32) {
        self.render(this.0, |_, pass| {
            pass.set_stencil_reference(reference);
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_begin_occlusion_query(&mut self, this: web::GpuRenderPassEncoder, query_index: u32) {
        self.render(this.0, |_, pass| {
            pass.begin_occlusion_query(query_index);
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_end_occlusion_query(&mut self, this: web::GpuRenderPassEncoder) {
        self.render(this.0, |_, pass| {
            pass.end_occlusion_query();
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_execute_bundles(&mut self, this: web::GpuRenderPassEncoder, bundles: &[web::GpuRenderBundle]) {
        self.render(this.0, |gpu, pass| {
            let bundles = bundles
                .iter()
                .map(|bundle| found(gpu.bundles.get(bundle.0, "render bundle")).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            pass.execute_bundles(&bundles);
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_end(&mut self, this: web::GpuRenderPassEncoder) {
        let device = self.device_of(Kind::RenderPass, this.0);
        if device.is_some_and(|device| self.device_lost(device)) {
            return;
        }
        self.wrapped(device, |gpu| found(gpu.passes.take(this.0, "render pass")).map(drop));
    }

    fn gpu_render_pass_encoder_push_debug_group(&mut self, this: web::GpuRenderPassEncoder, group_label: &str) {
        self.render(this.0, |_, pass| {
            pass.push_debug_group(group_label);
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_pop_debug_group(&mut self, this: web::GpuRenderPassEncoder) {
        self.render(this.0, |_, pass| {
            pass.pop_debug_group();
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_insert_debug_marker(&mut self, this: web::GpuRenderPassEncoder, marker_label: &str) {
        self.render(this.0, |_, pass| {
            pass.insert_debug_marker(marker_label);
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_set_bind_group(
        &mut self,
        this: web::GpuRenderPassEncoder,
        index: u32,
        bind_group: Option<web::GpuBindGroup>,
        dynamic_offsets_data: Option<&[u32]>,
        dynamic_offsets_data_start: Option<u64>,
        dynamic_offsets_data_length: Option<u32>,
    ) -> Result<(), web::SetBindGroupError> {
        let offsets = offsets(dynamic_offsets_data, dynamic_offsets_data_start, dynamic_offsets_data_length)?;
        self.render(this.0, |gpu, pass| {
            let group = bind_group
                .map(|group| found(gpu.groups.get(group.0, "bind group")).cloned())
                .transpose()?;
            pass.set_bind_group(index, group.as_ref(), offsets);
            Ok(())
        });
        Ok(())
    }

    fn gpu_render_pass_encoder_set_immediates(
        &mut self,
        this: web::GpuRenderPassEncoder,
        range_offset: u32,
        data: &[u8],
        data_offset: Option<u64>,
        data_size: Option<u64>,
    ) {
        match immediates(data, data_offset, data_size) {
            Ok(data) => self.render(this.0, |_, pass| {
                pass.set_immediates(range_offset, data);
                Ok(())
            }),
            Err(failure) => {
                let device = self.device_of(Kind::RenderPass, this.0);
                self.raise(device, failure);
            }
        }
    }

    fn gpu_render_pass_encoder_set_pipeline(&mut self, this: web::GpuRenderPassEncoder, pipeline: web::GpuRenderPipeline) {
        self.render(this.0, |gpu, pass| {
            let pipeline = found(gpu.pipelines.get(pipeline.0, "render pipeline"))?.clone();
            pass.set_pipeline(&pipeline);
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_set_index_buffer(
        &mut self,
        this: web::GpuRenderPassEncoder,
        buffer: web::GpuBuffer,
        index_format: web::GpuIndexFormat,
        offset: Option<u64>,
        size: Option<u64>,
    ) {
        self.render(this.0, |gpu, pass| {
            let buffer = found(gpu.buffers.get(buffer.0, "buffer"))?.clone();
            let slice = found(crate::slice(&buffer, offset.unwrap_or(0), size))?;
            pass.set_index_buffer(slice, index_format.convert());
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_set_vertex_buffer(
        &mut self,
        this: web::GpuRenderPassEncoder,
        slot: u32,
        buffer: Option<web::GpuBuffer>,
        offset: Option<u64>,
        size: Option<u64>,
    ) {
        self.render(this.0, |gpu, pass| {
            let buffer = buffer
                .map(|buffer| found(gpu.buffers.get(buffer.0, "buffer")).cloned())
                .transpose()?;
            let slice = found(crate::vertex_slice(buffer.as_ref(), slot, offset.unwrap_or(0), size))?;
            pass.set_vertex_buffer(slot, slice);
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_draw(&mut self, this: web::GpuRenderPassEncoder, vertex_count: u32, instance_count: Option<u32>, first_vertex: Option<u32>, first_instance: Option<u32>) {
        let (first_vertex, first_instance) = (first_vertex.unwrap_or(0), first_instance.unwrap_or(0));
        let instances = first_instance..first_instance.saturating_add(instance_count.unwrap_or(1));
        self.render(this.0, |_, pass| {
            pass.draw(first_vertex..first_vertex.saturating_add(vertex_count), instances);
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_draw_indexed(
        &mut self,
        this: web::GpuRenderPassEncoder,
        index_count: u32,
        instance_count: Option<u32>,
        first_index: Option<u32>,
        base_vertex: Option<i32>,
        first_instance: Option<u32>,
    ) {
        let (first_index, first_instance) = (first_index.unwrap_or(0), first_instance.unwrap_or(0));
        let instances = first_instance..first_instance.saturating_add(instance_count.unwrap_or(1));
        self.render(this.0, |_, pass| {
            pass.draw_indexed(first_index..first_index.saturating_add(index_count), base_vertex.unwrap_or(0), instances);
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_draw_indirect(&mut self, this: web::GpuRenderPassEncoder, indirect_buffer: web::GpuBuffer, indirect_offset: u64) {
        self.render(this.0, |gpu, pass| {
            let buffer = found(gpu.buffers.get(indirect_buffer.0, "buffer"))?.clone();
            pass.draw_indirect(&buffer, indirect_offset);
            Ok(())
        });
    }

    fn gpu_render_pass_encoder_draw_indexed_indirect(&mut self, this: web::GpuRenderPassEncoder, indirect_buffer: web::GpuBuffer, indirect_offset: u64) {
        self.render(this.0, |gpu, pass| {
            let buffer = found(gpu.buffers.get(indirect_buffer.0, "buffer"))?.clone();
            pass.draw_indexed_indirect(&buffer, indirect_offset);
            Ok(())
        });
    }

    fn gpu_render_bundle_encoder_finish(&mut self, this: web::GpuRenderBundleEncoder, descriptor: Option<web::GpuRenderBundleDescriptor>) -> web::GpuRenderBundle {
        let device = self.device_of(Kind::BundleEncoder, this.0);
        let label = descriptor.and_then(|descriptor| descriptor.label);
        let handle = self.make(device, Kind::Bundle, label.as_deref(), |gpu| &mut gpu.bundles, |gpu| {
            found(
                found(gpu.bundle_encoders.take(this.0, "render bundle encoder"))?
                    .finish(&gpu.device, label.as_deref()),
            )
        });
        web::GpuRenderBundle(handle)
    }

    fn gpu_render_bundle_encoder_push_debug_group(&mut self, this: web::GpuRenderBundleEncoder, _group_label: &str) {
        self.bundled(this.0, |_| Ok(crate::bundles::Command::PushDebugGroup));
    }

    fn gpu_render_bundle_encoder_pop_debug_group(&mut self, this: web::GpuRenderBundleEncoder) {
        self.bundled(this.0, |_| Ok(crate::bundles::Command::PopDebugGroup));
    }

    fn gpu_render_bundle_encoder_insert_debug_marker(&mut self, _this: web::GpuRenderBundleEncoder, _marker_label: &str) {
        // wgpu's bundle encoders have no markers, and a marker changes nothing drawn.
    }

    fn gpu_render_bundle_encoder_set_bind_group(
        &mut self,
        this: web::GpuRenderBundleEncoder,
        index: u32,
        bind_group: Option<web::GpuBindGroup>,
        dynamic_offsets_data: Option<&[u32]>,
        dynamic_offsets_data_start: Option<u64>,
        dynamic_offsets_data_length: Option<u32>,
    ) -> Result<(), web::SetBindGroupError> {
        let offsets = offsets(dynamic_offsets_data, dynamic_offsets_data_start, dynamic_offsets_data_length)?.to_vec();
        self.bundled(this.0, |gpu| {
            let group = bind_group
                .map(|group| found(gpu.groups.get(group.0, "bind group")).cloned())
                .transpose()?;
            Ok(crate::bundles::Command::BindGroup(index, group, offsets))
        });
        Ok(())
    }

    fn gpu_render_bundle_encoder_set_immediates(
        &mut self,
        this: web::GpuRenderBundleEncoder,
        range_offset: u32,
        data: &[u8],
        data_offset: Option<u64>,
        data_size: Option<u64>,
    ) {
        let data = immediates(data, data_offset, data_size).map(<[u8]>::to_vec);
        self.bundled(this.0, |_| Ok(crate::bundles::Command::Immediates(range_offset, data?)));
    }

    fn gpu_render_bundle_encoder_set_pipeline(&mut self, this: web::GpuRenderBundleEncoder, pipeline: web::GpuRenderPipeline) {
        self.bundled(this.0, |gpu| {
            Ok(crate::bundles::Command::Pipeline(found(gpu.pipelines.get(pipeline.0, "render pipeline"))?.clone()))
        });
    }

    fn gpu_render_bundle_encoder_set_index_buffer(
        &mut self,
        this: web::GpuRenderBundleEncoder,
        buffer: web::GpuBuffer,
        index_format: web::GpuIndexFormat,
        offset: Option<u64>,
        size: Option<u64>,
    ) {
        self.bundled(this.0, |gpu| {
            let buffer = found(gpu.buffers.get(buffer.0, "buffer"))?.clone();
            Ok(crate::bundles::Command::IndexBuffer(buffer, index_format.convert(), offset.unwrap_or(0), size))
        });
    }

    fn gpu_render_bundle_encoder_set_vertex_buffer(
        &mut self,
        this: web::GpuRenderBundleEncoder,
        slot: u32,
        buffer: Option<web::GpuBuffer>,
        offset: Option<u64>,
        size: Option<u64>,
    ) {
        self.bundled(this.0, |gpu| {
            let buffer = buffer
                .map(|buffer| found(gpu.buffers.get(buffer.0, "buffer")).cloned())
                .transpose()?;
            Ok(crate::bundles::Command::VertexBuffer(slot, buffer, offset.unwrap_or(0), size))
        });
    }

    fn gpu_render_bundle_encoder_draw(&mut self, this: web::GpuRenderBundleEncoder, vertex_count: u32, instance_count: Option<u32>, first_vertex: Option<u32>, first_instance: Option<u32>) {
        let (first_vertex, first_instance) = (first_vertex.unwrap_or(0), first_instance.unwrap_or(0));
        self.bundled(this.0, |_| {
            Ok(crate::bundles::Command::Draw(
                first_vertex..first_vertex.saturating_add(vertex_count),
                first_instance..first_instance.saturating_add(instance_count.unwrap_or(1)),
            ))
        });
    }

    fn gpu_render_bundle_encoder_draw_indexed(
        &mut self,
        this: web::GpuRenderBundleEncoder,
        index_count: u32,
        instance_count: Option<u32>,
        first_index: Option<u32>,
        base_vertex: Option<i32>,
        first_instance: Option<u32>,
    ) {
        let (first_index, first_instance) = (first_index.unwrap_or(0), first_instance.unwrap_or(0));
        self.bundled(this.0, |_| {
            Ok(crate::bundles::Command::DrawIndexed(
                first_index..first_index.saturating_add(index_count),
                base_vertex.unwrap_or(0),
                first_instance..first_instance.saturating_add(instance_count.unwrap_or(1)),
            ))
        });
    }

    fn gpu_render_bundle_encoder_draw_indirect(&mut self, this: web::GpuRenderBundleEncoder, indirect_buffer: web::GpuBuffer, indirect_offset: u64) {
        self.bundled(this.0, |gpu| {
            let buffer = found(gpu.buffers.get(indirect_buffer.0, "buffer"))?.clone();
            Ok(crate::bundles::Command::DrawIndirect(buffer, indirect_offset))
        });
    }

    fn gpu_render_bundle_encoder_draw_indexed_indirect(&mut self, this: web::GpuRenderBundleEncoder, indirect_buffer: web::GpuBuffer, indirect_offset: u64) {
        self.bundled(this.0, |gpu| {
            let buffer = found(gpu.buffers.get(indirect_buffer.0, "buffer"))?.clone();
            Ok(crate::bundles::Command::DrawIndexedIndirect(buffer, indirect_offset))
        });
    }

    fn gpu_queue_submit(&mut self, this: web::GpuQueue, command_buffers: &[web::GpuCommandBuffer]) {
        let device = self.web.queues.get(this.0, "queue").ok().copied();
        if device.is_some_and(|device| self.device_lost(device)) {
            return;
        }
        self.wrapped(device, |gpu| {
            let mut buffers = Vec::with_capacity(command_buffers.len());
            for buffer in command_buffers {
                buffers.push(found(gpu.command_buffers.take(buffer.0, "command buffer"))?);
            }
            gpu.queue.submit(buffers);
            Ok(())
        });
    }

    fn gpu_queue_on_submitted_work_done(&mut self, _this: web::GpuQueue, settle: Promise<()>) {
        let ticket = self.settling.work_done(&self.device, &self.queue);
        self.web.work.insert(ticket, settle);
    }

    fn gpu_queue_write_buffer_with_copy(
        &mut self,
        this: web::GpuQueue,
        buffer: web::GpuBuffer,
        buffer_offset: u64,
        data: &[u8],
        data_offset: Option<u64>,
        size: Option<u64>,
    ) -> Result<(), web::WriteBufferError> {
        let start = data_offset.unwrap_or(0);
        let written = usize::try_from(start)
            .ok()
            .and_then(|start| {
                let end = match size {
                    Some(size) => start.checked_add(usize::try_from(size).ok()?)?,
                    None => data.len(),
                };
                data.get(start..end)
            })
            .ok_or_else(|| web::WriteBufferError {
                kind: web::WriteBufferErrorKind::OperationError,
                message: format!("the data is {} bytes, shorter than the range asked of it", data.len()),
            })?;
        let device = self.web.queues.get(this.0, "queue").ok().copied();
        if device.is_some_and(|device| self.device_lost(device)) {
            return Ok(());
        }
        self.wrapped(device, |gpu| {
            let buffer = found(gpu.buffers.get(buffer.0, "buffer"))?;
            gpu.queue.write_buffer(buffer, buffer_offset, written);
            Ok(())
        });
        Ok(())
    }

    fn gpu_queue_write_texture_with_copy(
        &mut self,
        this: web::GpuQueue,
        destination: web::GpuTexelCopyTextureInfo,
        data: &[u8],
        data_layout: web::GpuTexelCopyBufferLayout,
        size: web::GpuExtent3D,
    ) {
        let device = self.web.queues.get(this.0, "queue").ok().copied();
        if device.is_some_and(|device| self.device_lost(device)) {
            return;
        }
        self.wrapped(device, |gpu| {
            let texture = found(gpu.textures.get(destination.texture.0, "texture"))?;
            gpu.queue.write_texture(
                texture_info(texture, &destination),
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: data_layout.offset.unwrap_or(0),
                    bytes_per_row: data_layout.bytes_per_row,
                    rows_per_image: data_layout.rows_per_image,
                },
                size.convert(),
            );
            Ok(())
        });
    }

    fn gpu_canvas_context_configure(&mut self, this: web::GpuCanvasContext, configuration: web::GpuCanvasConfiguration) {
        self.configure_context(this.0, configuration);
    }

    fn gpu_canvas_context_unconfigure(&mut self, this: web::GpuCanvasContext) {
        self.unconfigure_context(this.0);
    }

    fn gpu_canvas_context_get_configuration(&mut self, this: web::GpuCanvasContext) -> Option<web::GpuCanvasConfigurationOwned> {
        self.web
            .contexts
            .get(this.0, "canvas context")
            .ok()
            .and_then(|context| context.configuration.clone())
    }

    fn gpu_canvas_context_get_current_texture(&mut self, this: web::GpuCanvasContext) -> web::GpuTexture {
        web::GpuTexture(self.current_texture(this.0))
    }

    fn settle(&mut self, wait: bool) -> Vec<Settled> {
        self.settle_web(wait)
    }

    fn gpu_drop(&mut self, this: web::Gpu) {
        self.web.gpus.remove(this.0);
    }

    fn gpu_adapter_drop(&mut self, this: web::GpuAdapter) {
        self.web.adapters.remove(this.0);
    }

    fn gpu_adapter_info_drop(&mut self, this: web::GpuAdapterInfo) {
        self.web.infos.remove(this.0);
    }

    fn gpu_bind_group_drop(&mut self, this: web::GpuBindGroup) {
        self.drop_object(Kind::Group, this.0, |gpu| &mut gpu.groups);
    }

    fn gpu_bind_group_layout_drop(&mut self, this: web::GpuBindGroupLayout) {
        self.drop_object(Kind::GroupLayout, this.0, |gpu| &mut gpu.group_layouts);
    }

    fn gpu_buffer_drop(&mut self, this: web::GpuBuffer) {
        self.settling.forget(this.0);
        self.web.buffers.remove(&this.0);
        self.drop_object(Kind::Buffer, this.0, |gpu| &mut gpu.buffers);
    }

    fn gpu_canvas_context_drop(&mut self, this: web::GpuCanvasContext) {
        self.drop_context(this.0);
    }

    fn gpu_command_buffer_drop(&mut self, this: web::GpuCommandBuffer) {
        self.drop_object(Kind::CommandBuffer, this.0, |gpu| &mut gpu.command_buffers);
    }

    fn gpu_command_encoder_drop(&mut self, this: web::GpuCommandEncoder) {
        self.drop_object(Kind::Encoder, this.0, |gpu| &mut gpu.encoders);
    }

    fn gpu_compilation_info_drop(&mut self, this: web::GpuCompilationInfo) {
        self.web.compilation_infos.remove(this.0);
    }

    fn gpu_compilation_message_drop(&mut self, this: web::GpuCompilationMessage) {
        self.web.messages.remove(this.0);
    }

    fn gpu_compute_pass_encoder_drop(&mut self, this: web::GpuComputePassEncoder) {
        self.drop_object(Kind::ComputePass, this.0, |gpu| &mut gpu.compute_passes);
    }

    fn gpu_compute_pipeline_drop(&mut self, this: web::GpuComputePipeline) {
        self.drop_object(Kind::ComputePipeline, this.0, |gpu| &mut gpu.compute_pipelines);
    }

    fn gpu_device_drop(&mut self, this: web::GpuDevice) {
        // The device lives on in what was made on it, as a JS object does while anything
        // refers to it; only its name goes.
        self.web.labels.remove(&(Kind::Device, this.0));
    }

    fn gpu_device_lost_info_drop(&mut self, this: web::GpuDeviceLostInfo) {
        self.web.lost_infos.remove(this.0);
    }

    fn gpu_error_drop(&mut self, this: web::GpuError) {
        self.web.errors.remove(this.0);
    }

    fn gpu_pipeline_layout_drop(&mut self, this: web::GpuPipelineLayout) {
        self.drop_object(Kind::PipelineLayout, this.0, |gpu| &mut gpu.pipeline_layouts);
    }

    fn gpu_query_set_drop(&mut self, this: web::GpuQuerySet) {
        self.web.query_sets.remove(&this.0);
        self.drop_object(Kind::QuerySet, this.0, |gpu| &mut gpu.query_sets);
    }

    fn gpu_queue_drop(&mut self, _this: web::GpuQueue) {
        // A device's queue is the same object however often it is asked for.
    }

    fn gpu_render_bundle_drop(&mut self, this: web::GpuRenderBundle) {
        self.drop_object(Kind::Bundle, this.0, |gpu| &mut gpu.bundles);
    }

    fn gpu_render_bundle_encoder_drop(&mut self, this: web::GpuRenderBundleEncoder) {
        self.drop_object(Kind::BundleEncoder, this.0, |gpu| &mut gpu.bundle_encoders);
    }

    fn gpu_render_pass_encoder_drop(&mut self, this: web::GpuRenderPassEncoder) {
        self.drop_object(Kind::RenderPass, this.0, |gpu| &mut gpu.passes);
    }

    fn gpu_render_pipeline_drop(&mut self, this: web::GpuRenderPipeline) {
        self.drop_object(Kind::RenderPipeline, this.0, |gpu| &mut gpu.pipelines);
    }

    fn gpu_sampler_drop(&mut self, this: web::GpuSampler) {
        self.drop_object(Kind::Sampler, this.0, |gpu| &mut gpu.samplers);
    }

    fn gpu_shader_module_drop(&mut self, this: web::GpuShaderModule) {
        self.drop_object(Kind::Module, this.0, |gpu| &mut gpu.modules);
    }

    fn gpu_supported_features_drop(&mut self, this: web::GpuSupportedFeatures) {
        self.web.features.remove(this.0);
    }

    fn gpu_supported_limits_drop(&mut self, this: web::GpuSupportedLimits) {
        self.web.limits.remove(this.0);
    }

    fn gpu_texture_drop(&mut self, this: web::GpuTexture) {
        self.made_textures.remove(&this.0);
        self.drop_object(Kind::Texture, this.0, |gpu| &mut gpu.textures);
    }

    fn gpu_texture_view_drop(&mut self, this: web::GpuTextureView) {
        self.drop_object(Kind::View, this.0, |gpu| &mut gpu.views);
    }

    fn gpu_uncaptured_error_event_drop(&mut self, this: web::GpuUncapturedErrorEvent) {
        self.web.events.remove(this.0);
    }

    fn wgsl_language_features_drop(&mut self, this: web::WgslLanguageFeatures) {
        self.web.wgsl.remove(this.0);
    }
}

/// The dynamic offsets `setBindGroup` names: `length` of them from `start`, or all of them.
fn offsets(data: Option<&[u32]>, start: Option<u64>, length: Option<u32>) -> Result<&[u32], web::SetBindGroupError> {
    let data = data.unwrap_or(&[]);
    let start = start.and_then(|start| usize::try_from(start).ok()).unwrap_or(0);
    let range = match length {
        Some(length) => start.checked_add(length as usize).map(|end| start..end),
        None => Some(start..data.len()),
    };
    range.and_then(|range| data.get(range)).ok_or_else(|| web::SetBindGroupError {
        kind: web::SetBindGroupErrorKind::RangeError,
        message: format!("the dynamic offsets asked for run past the {} given", data.len()),
    })
}

fn immediates(data: &[u8], offset: Option<u64>, size: Option<u64>) -> Result<&[u8], Failure> {
    let start = offset.and_then(|offset| usize::try_from(offset).ok()).unwrap_or(0);
    let end = match size {
        Some(size) => usize::try_from(size).ok().and_then(|size| start.checked_add(size)),
        None => Some(data.len()),
    };
    end.and_then(|end| data.get(start..end))
        .ok_or_else(|| Failure::validation("the immediate data asked for runs past the data given"))
}

pub(super) fn texture_info<'a>(texture: &'a wgpu::Texture, info: &web::GpuTexelCopyTextureInfo) -> wgpu::TexelCopyTextureInfo<'a> {
    wgpu::TexelCopyTextureInfo {
        texture,
        mip_level: info.mip_level.unwrap_or(0),
        origin: info.origin.convert(),
        aspect: info.aspect.map_or(wgpu::TextureAspect::All, Convert::convert),
    }
}

fn buffer_info<'a>(buffer: &'a wgpu::Buffer, info: &web::GpuTexelCopyBufferInfo) -> wgpu::TexelCopyBufferInfo<'a> {
    wgpu::TexelCopyBufferInfo {
        buffer,
        layout: wgpu::TexelCopyBufferLayout {
            offset: info.offset.unwrap_or(0),
            bytes_per_row: info.bytes_per_row,
            rows_per_image: info.rows_per_image,
        },
    }
}
