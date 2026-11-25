use be_wasm_gpu_abi::device as abi;

pub(crate) fn feature(name: abi::FeatureName) -> wgpu::Features {
    use abi::FeatureName as Name;
    match name {
        Name::DepthClipControl => wgpu::Features::DEPTH_CLIP_CONTROL,
        Name::Depth32floatStencil8 => wgpu::Features::DEPTH32FLOAT_STENCIL8,
        Name::TextureCompressionBc => wgpu::Features::TEXTURE_COMPRESSION_BC,
        Name::TextureCompressionBcSliced3d => wgpu::Features::TEXTURE_COMPRESSION_BC_SLICED_3D,
        Name::TextureCompressionEtc2 => wgpu::Features::TEXTURE_COMPRESSION_ETC2,
        Name::TextureCompressionAstc => wgpu::Features::TEXTURE_COMPRESSION_ASTC,
        Name::TextureCompressionAstcSliced3d => wgpu::Features::TEXTURE_COMPRESSION_ASTC_SLICED_3D,
        Name::TimestampQuery => wgpu::Features::TIMESTAMP_QUERY,
        Name::IndirectFirstInstance => wgpu::Features::INDIRECT_FIRST_INSTANCE,
        Name::ShaderF16 => wgpu::Features::SHADER_F16,
        Name::Rg11b10ufloatRenderable => wgpu::Features::RG11B10UFLOAT_RENDERABLE,
        Name::Bgra8unormStorage => wgpu::Features::BGRA8UNORM_STORAGE,
        Name::Float32Filterable => wgpu::Features::FLOAT32_FILTERABLE,
        Name::Float32Blendable => wgpu::Features::FLOAT32_BLENDABLE,
        Name::DualSourceBlending => wgpu::Features::DUAL_SOURCE_BLENDING,
        Name::ClipDistances => wgpu::Features::CLIP_DISTANCES,
        Name::PrimitiveIndex => wgpu::Features::PRIMITIVE_INDEX,
    }
}

pub(crate) fn features(names: &[abi::FeatureName]) -> wgpu::Features {
    names
        .iter()
        .fold(wgpu::Features::empty(), |features, name| {
            features | feature(*name)
        })
}

pub(crate) fn limits(limits: &abi::SupportedLimits) -> wgpu::Limits {
    wgpu::Limits {
        max_texture_dimension_1d: limits.max_texture_dimension1d,
        max_texture_dimension_2d: limits.max_texture_dimension2d,
        max_texture_dimension_3d: limits.max_texture_dimension3d,
        max_texture_array_layers: limits.max_texture_array_layers,
        max_bind_groups: limits.max_bind_groups,
        max_bindings_per_bind_group: limits.max_bindings_per_bind_group,
        max_dynamic_uniform_buffers_per_pipeline_layout: limits
            .max_dynamic_uniform_buffers_per_pipeline_layout,
        max_dynamic_storage_buffers_per_pipeline_layout: limits
            .max_dynamic_storage_buffers_per_pipeline_layout,
        max_sampled_textures_per_shader_stage: limits.max_sampled_textures_per_shader_stage,
        max_samplers_per_shader_stage: limits.max_samplers_per_shader_stage,
        max_storage_buffers_per_shader_stage: limits.max_storage_buffers_per_shader_stage,
        max_storage_textures_per_shader_stage: limits.max_storage_textures_per_shader_stage,
        max_uniform_buffers_per_shader_stage: limits.max_uniform_buffers_per_shader_stage,
        max_binding_array_elements_per_shader_stage: 0,
        max_binding_array_sampler_elements_per_shader_stage: 0,
        max_binding_array_acceleration_structure_elements_per_shader_stage: 0,
        max_uniform_buffer_binding_size: limits.max_uniform_buffer_binding_size,
        max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
        max_vertex_buffers: limits.max_vertex_buffers,
        max_buffer_size: limits.max_buffer_size,
        max_vertex_attributes: limits.max_vertex_attributes,
        max_vertex_buffer_array_stride: limits.max_vertex_buffer_array_stride,
        max_inter_stage_shader_variables: limits.max_inter_stage_shader_variables,
        min_uniform_buffer_offset_alignment: limits.min_uniform_buffer_offset_alignment,
        min_storage_buffer_offset_alignment: limits.min_storage_buffer_offset_alignment,
        max_color_attachments: limits.max_color_attachments,
        max_color_attachment_bytes_per_sample: limits.max_color_attachment_bytes_per_sample,
        max_compute_workgroup_storage_size: limits.max_compute_workgroup_storage_size,
        max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
        max_compute_workgroup_size_x: limits.max_compute_workgroup_size_x,
        max_compute_workgroup_size_y: limits.max_compute_workgroup_size_y,
        max_compute_workgroup_size_z: limits.max_compute_workgroup_size_z,
        max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
        ..wgpu::Limits::default()
    }
}

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

pub(crate) use be_wasm_gpu_convert::allowed::Allowed;
