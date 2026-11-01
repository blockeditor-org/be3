use block_gpu_abi::device as abi;

pub(crate) fn constants(values: &[(String, f64)]) -> Vec<(&str, f64)> {
    values
        .iter()
        .map(|(key, value)| (key.as_str(), *value))
        .collect()
}

pub(crate) fn label(value: &Option<String>) -> Option<&str> {
    value.as_deref()
}

pub(crate) fn size(value: Option<u64>) -> Option<wgpu::BufferSize> {
    value.and_then(wgpu::BufferSize::new)
}

pub(crate) fn limits(value: &wgpu::Limits) -> abi::SupportedLimits {
    abi::SupportedLimits {
        max_texture_dimension1d: value.max_texture_dimension_1d,
        max_texture_dimension2d: value.max_texture_dimension_2d,
        max_texture_dimension3d: value.max_texture_dimension_3d,
        max_texture_array_layers: value.max_texture_array_layers,
        max_bind_groups: value.max_bind_groups,
        max_bindings_per_bind_group: value.max_bindings_per_bind_group,
        max_sampled_textures_per_shader_stage: value.max_sampled_textures_per_shader_stage,
        max_samplers_per_shader_stage: value.max_samplers_per_shader_stage,
        max_uniform_buffers_per_shader_stage: value.max_uniform_buffers_per_shader_stage,
        max_uniform_buffer_binding_size: value.max_uniform_buffer_binding_size,
        max_storage_buffer_binding_size: value.max_storage_buffer_binding_size,
        max_vertex_buffers: value.max_vertex_buffers,
        max_buffer_size: value.max_buffer_size,
        max_vertex_attributes: value.max_vertex_attributes,
        max_vertex_buffer_array_stride: value.max_vertex_buffer_array_stride,
        min_uniform_buffer_offset_alignment: value.min_uniform_buffer_offset_alignment,
        min_storage_buffer_offset_alignment: value.min_storage_buffer_offset_alignment,
        max_dynamic_uniform_buffers_per_pipeline_layout: value
            .max_dynamic_uniform_buffers_per_pipeline_layout,
        max_dynamic_storage_buffers_per_pipeline_layout: value
            .max_dynamic_storage_buffers_per_pipeline_layout,
        max_storage_buffers_per_shader_stage: value.max_storage_buffers_per_shader_stage,
        max_storage_textures_per_shader_stage: value.max_storage_textures_per_shader_stage,
        max_inter_stage_shader_variables: value.max_inter_stage_shader_variables,
        max_color_attachments: value.max_color_attachments,
        max_color_attachment_bytes_per_sample: value.max_color_attachment_bytes_per_sample,
        max_compute_workgroup_storage_size: value.max_compute_workgroup_storage_size,
        max_compute_invocations_per_workgroup: value.max_compute_invocations_per_workgroup,
        max_compute_workgroup_size_x: value.max_compute_workgroup_size_x,
        max_compute_workgroup_size_y: value.max_compute_workgroup_size_y,
        max_compute_workgroup_size_z: value.max_compute_workgroup_size_z,
        max_compute_workgroups_per_dimension: value.max_compute_workgroups_per_dimension,
    }
}

pub(crate) const FEATURES: [(wgpu::Features, abi::FeatureName); 17] = [
    (
        wgpu::Features::DEPTH_CLIP_CONTROL,
        abi::FeatureName::DepthClipControl,
    ),
    (
        wgpu::Features::DEPTH32FLOAT_STENCIL8,
        abi::FeatureName::Depth32floatStencil8,
    ),
    (
        wgpu::Features::TEXTURE_COMPRESSION_BC,
        abi::FeatureName::TextureCompressionBc,
    ),
    (
        wgpu::Features::TEXTURE_COMPRESSION_BC_SLICED_3D,
        abi::FeatureName::TextureCompressionBcSliced3d,
    ),
    (
        wgpu::Features::TEXTURE_COMPRESSION_ETC2,
        abi::FeatureName::TextureCompressionEtc2,
    ),
    (
        wgpu::Features::TEXTURE_COMPRESSION_ASTC,
        abi::FeatureName::TextureCompressionAstc,
    ),
    (
        wgpu::Features::TEXTURE_COMPRESSION_ASTC_SLICED_3D,
        abi::FeatureName::TextureCompressionAstcSliced3d,
    ),
    (
        wgpu::Features::TIMESTAMP_QUERY,
        abi::FeatureName::TimestampQuery,
    ),
    (
        wgpu::Features::INDIRECT_FIRST_INSTANCE,
        abi::FeatureName::IndirectFirstInstance,
    ),
    (wgpu::Features::SHADER_F16, abi::FeatureName::ShaderF16),
    (
        wgpu::Features::RG11B10UFLOAT_RENDERABLE,
        abi::FeatureName::Rg11b10ufloatRenderable,
    ),
    (
        wgpu::Features::BGRA8UNORM_STORAGE,
        abi::FeatureName::Bgra8unormStorage,
    ),
    (
        wgpu::Features::FLOAT32_FILTERABLE,
        abi::FeatureName::Float32Filterable,
    ),
    (
        wgpu::Features::FLOAT32_BLENDABLE,
        abi::FeatureName::Float32Blendable,
    ),
    (
        wgpu::Features::DUAL_SOURCE_BLENDING,
        abi::FeatureName::DualSourceBlending,
    ),
    (
        wgpu::Features::CLIP_DISTANCES,
        abi::FeatureName::ClipDistances,
    ),
    (
        wgpu::Features::PRIMITIVE_INDEX,
        abi::FeatureName::PrimitiveIndex,
    ),
];

pub(crate) fn features(value: wgpu::Features) -> Vec<abi::FeatureName> {
    FEATURES
        .iter()
        .filter(|(feature, _)| value.contains(*feature))
        .map(|(_, name)| *name)
        .collect()
}
