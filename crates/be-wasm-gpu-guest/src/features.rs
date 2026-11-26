use be_wasm_gpu_abi::guest::webgpu as gpu;
use be_wasm_gpu_abi::webgpu::webgpu as web;
use be_wasm_gpu_convert::webgpu::{FEATURES, feature, feature_name};

/// The wgpu features an adapter's or a device's supported features make.
pub(crate) fn features(supported: &gpu::GpuSupportedFeatures) -> wgpu::Features {
    FEATURES
        .into_iter()
        .filter(|name| supported.has(feature_name(*name)))
        .filter_map(feature)
        .fold(wgpu::Features::empty(), |features, feature| features | feature)
}

pub(crate) fn limits(limits: &gpu::GpuSupportedLimits) -> wgpu::Limits {
    wgpu::Limits {
        max_texture_dimension_1d: limits.max_texture_dimension1_d(),
        max_texture_dimension_2d: limits.max_texture_dimension2_d(),
        max_texture_dimension_3d: limits.max_texture_dimension3_d(),
        max_texture_array_layers: limits.max_texture_array_layers(),
        max_bind_groups: limits.max_bind_groups(),
        max_bind_groups_plus_vertex_buffers: limits.max_bind_groups_plus_vertex_buffers(),
        max_bindings_per_bind_group: limits.max_bindings_per_bind_group(),
        max_dynamic_uniform_buffers_per_pipeline_layout: limits
            .max_dynamic_uniform_buffers_per_pipeline_layout(),
        max_dynamic_storage_buffers_per_pipeline_layout: limits
            .max_dynamic_storage_buffers_per_pipeline_layout(),
        max_sampled_textures_per_shader_stage: limits.max_sampled_textures_per_shader_stage(),
        max_samplers_per_shader_stage: limits.max_samplers_per_shader_stage(),
        max_storage_buffers_per_shader_stage: limits.max_storage_buffers_per_shader_stage(),
        max_storage_textures_per_shader_stage: limits.max_storage_textures_per_shader_stage(),
        max_uniform_buffers_per_shader_stage: limits.max_uniform_buffers_per_shader_stage(),
        max_binding_array_elements_per_shader_stage: 0,
        max_binding_array_sampler_elements_per_shader_stage: 0,
        max_binding_array_acceleration_structure_elements_per_shader_stage: 0,
        max_uniform_buffer_binding_size: limits.max_uniform_buffer_binding_size(),
        max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size(),
        max_vertex_buffers: limits.max_vertex_buffers(),
        max_buffer_size: limits.max_buffer_size(),
        max_vertex_attributes: limits.max_vertex_attributes(),
        max_vertex_buffer_array_stride: limits.max_vertex_buffer_array_stride(),
        max_inter_stage_shader_variables: limits.max_inter_stage_shader_variables(),
        min_uniform_buffer_offset_alignment: limits.min_uniform_buffer_offset_alignment(),
        min_storage_buffer_offset_alignment: limits.min_storage_buffer_offset_alignment(),
        max_color_attachments: limits.max_color_attachments(),
        max_color_attachment_bytes_per_sample: limits.max_color_attachment_bytes_per_sample(),
        max_compute_workgroup_storage_size: limits.max_compute_workgroup_storage_size(),
        max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup(),
        max_compute_workgroup_size_x: limits.max_compute_workgroup_size_x(),
        max_compute_workgroup_size_y: limits.max_compute_workgroup_size_y(),
        max_compute_workgroup_size_z: limits.max_compute_workgroup_size_z(),
        max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension(),
        max_immediate_size: limits.max_immediate_size(),
        ..wgpu::Limits::default()
    }
}

/// Refuses a device the adapter cannot give, before asking the host, as wgpu's own backends do.
pub(crate) fn requested(
    descriptor: &wgpu::DeviceDescriptor<'_>,
    features: wgpu::Features,
    limits: &wgpu::Limits,
) -> Result<(), String> {
    let unsupported = descriptor.required_features & !features;
    if !unsupported.is_empty() {
        return Err(format!(
            "Unsupported features were requested: {unsupported:?}"
        ));
    }
    let mut exceeded = None;
    descriptor.required_limits.check_limits_with_fail_fn(
        limits,
        true,
        |name, requested, allowed| {
            exceeded = Some(format!(
                "Limit '{name}' value {requested} is better than allowed {allowed}"
            ));
        },
    );
    exceeded.map_or(Ok(()), Err)
}

/// A device descriptor for the host, and the record of limits it names, which must live until
/// the request is made.
pub(crate) struct Asked {
    pub(crate) descriptor: web::GpuDeviceDescriptor,
    pub(crate) limits: gpu::RecordOptionGpuSize64,
}

/// What `descriptor` asks of the host: its features by WebGPU's names, and each limit better
/// than WebGPU's default, under its WebGPU name.
pub(crate) fn descriptor(descriptor: &wgpu::DeviceDescriptor<'_>) -> Asked {
    let names = FEATURES
        .into_iter()
        .filter(|name| {
            feature(*name).is_some_and(|wanted| {
                !wanted.is_empty() && descriptor.required_features.contains(wanted)
            })
        })
        .collect();
    let record = gpu::RecordOptionGpuSize64::new();
    let asked = &descriptor.required_limits;
    let defaults = wgpu::Limits::default();
    macro_rules! limits {
        ($($name:literal => $field:ident),* $(,)?) => {
            $(
                let better = if $name.starts_with("min") {
                    asked.$field < defaults.$field
                } else {
                    asked.$field > defaults.$field
                };
                if better {
                    record.add($name, Some(u64::from(asked.$field)));
                }
            )*
        };
    }
    limits! {
        "maxTextureDimension1D" => max_texture_dimension_1d,
        "maxTextureDimension2D" => max_texture_dimension_2d,
        "maxTextureDimension3D" => max_texture_dimension_3d,
        "maxTextureArrayLayers" => max_texture_array_layers,
        "maxBindGroups" => max_bind_groups,
        "maxBindGroupsPlusVertexBuffers" => max_bind_groups_plus_vertex_buffers,
        "maxBindingsPerBindGroup" => max_bindings_per_bind_group,
        "maxDynamicUniformBuffersPerPipelineLayout" => max_dynamic_uniform_buffers_per_pipeline_layout,
        "maxDynamicStorageBuffersPerPipelineLayout" => max_dynamic_storage_buffers_per_pipeline_layout,
        "maxSampledTexturesPerShaderStage" => max_sampled_textures_per_shader_stage,
        "maxSamplersPerShaderStage" => max_samplers_per_shader_stage,
        "maxStorageBuffersPerShaderStage" => max_storage_buffers_per_shader_stage,
        "maxStorageTexturesPerShaderStage" => max_storage_textures_per_shader_stage,
        "maxUniformBuffersPerShaderStage" => max_uniform_buffers_per_shader_stage,
        "maxUniformBufferBindingSize" => max_uniform_buffer_binding_size,
        "maxStorageBufferBindingSize" => max_storage_buffer_binding_size,
        "minUniformBufferOffsetAlignment" => min_uniform_buffer_offset_alignment,
        "minStorageBufferOffsetAlignment" => min_storage_buffer_offset_alignment,
        "maxVertexBuffers" => max_vertex_buffers,
        "maxBufferSize" => max_buffer_size,
        "maxVertexAttributes" => max_vertex_attributes,
        "maxVertexBufferArrayStride" => max_vertex_buffer_array_stride,
        "maxInterStageShaderVariables" => max_inter_stage_shader_variables,
        "maxColorAttachments" => max_color_attachments,
        "maxColorAttachmentBytesPerSample" => max_color_attachment_bytes_per_sample,
        "maxComputeWorkgroupStorageSize" => max_compute_workgroup_storage_size,
        "maxComputeInvocationsPerWorkgroup" => max_compute_invocations_per_workgroup,
        "maxComputeWorkgroupSizeX" => max_compute_workgroup_size_x,
        "maxComputeWorkgroupSizeY" => max_compute_workgroup_size_y,
        "maxComputeWorkgroupSizeZ" => max_compute_workgroup_size_z,
        "maxComputeWorkgroupsPerDimension" => max_compute_workgroups_per_dimension,
        "maxImmediateSize" => max_immediate_size,
    }
    Asked {
        descriptor: web::GpuDeviceDescriptor {
            required_features: Some(names),
            required_limits: Some(record.handle()),
            default_queue: None,
            label: descriptor.label.map(str::to_owned),
        },
        limits: record,
    }
}
