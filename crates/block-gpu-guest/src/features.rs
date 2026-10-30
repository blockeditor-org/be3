use block_gpu_abi::device as abi;

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

#[derive(Debug, Clone, Copy)]
pub(crate) struct Allowed {
    pub(crate) requested: wgpu::Features,
    pub(crate) offered: wgpu::Features,
}

impl Allowed {
    pub(crate) fn query_set(&self, kind: wgpu::QueryType) -> Result<(), String> {
        match kind {
            wgpu::QueryType::Timestamp => self.needs(wgpu::Features::TIMESTAMP_QUERY),
            _ => Ok(()),
        }
    }

    fn needs(&self, needed: wgpu::Features) -> Result<(), String> {
        let missing = needed & !self.requested;
        match missing.is_empty() {
            true => Ok(()),
            false => Err(format!(
                "Features {missing:?} are required but not enabled on the device"
            )),
        }
    }

    fn format_features(&self, format: wgpu::TextureFormat) -> [wgpu::TextureFormatFeatures; 2] {
        [
            format.guaranteed_format_features(self.requested),
            format.guaranteed_format_features(self.offered),
        ]
    }

    pub(crate) fn texture(&self, descriptor: &wgpu::TextureDescriptor<'_>) -> Result<(), String> {
        for format in std::iter::once(&descriptor.format).chain(descriptor.view_formats) {
            self.needs(format.required_features())?;
        }
        let [requested, offered] = self.format_features(descriptor.format);
        let refused = descriptor
            .usage
            .difference(requested.allowed_usages)
            .intersection(offered.allowed_usages);
        match refused.is_empty() {
            true => Ok(()),
            false => Err(format!(
                "Texture usages {refused:?} of format {:?} need a feature that is not enabled on the device",
                descriptor.format
            )),
        }
    }

    pub(crate) fn group_layout(
        &self,
        descriptor: &wgpu::BindGroupLayoutDescriptor<'_>,
    ) -> Result<(), String> {
        for entry in descriptor.entries {
            if let wgpu::BindingType::StorageTexture { format, .. } = entry.ty {
                self.needs(format.required_features())?;
                let [requested, offered] = self.format_features(format);
                let storage = wgpu::TextureUsages::STORAGE_BINDING;
                if !requested.allowed_usages.contains(storage)
                    && offered.allowed_usages.contains(storage)
                {
                    return Err(format!(
                        "Storage textures of format {format:?} need a feature that is not enabled on the device"
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn render_pipeline(
        &self,
        descriptor: &wgpu::RenderPipelineDescriptor<'_>,
    ) -> Result<(), String> {
        if descriptor.primitive.unclipped_depth {
            self.needs(wgpu::Features::DEPTH_CLIP_CONTROL)?;
        }
        let Some(fragment) = &descriptor.fragment else {
            return Ok(());
        };
        fragment
            .targets
            .iter()
            .flatten()
            .try_for_each(|target| self.color_target(target))
    }

    pub(crate) fn color_target(&self, target: &wgpu::ColorTargetState) -> Result<(), String> {
        let Some(blend) = target.blend else {
            return Ok(());
        };
        let factors = [
            blend.color.src_factor,
            blend.color.dst_factor,
            blend.alpha.src_factor,
            blend.alpha.dst_factor,
        ];
        if factors
            .iter()
            .any(wgpu::BlendFactor::ref_second_blend_source)
        {
            self.needs(wgpu::Features::DUAL_SOURCE_BLENDING)?;
        }
        let [requested, offered] = self.format_features(target.format);
        let blendable = wgpu::TextureFormatFeatureFlags::BLENDABLE;
        match !requested.flags.contains(blendable) && offered.flags.contains(blendable) {
            true => Err(format!(
                "Blending on format {:?} needs a feature that is not enabled on the device",
                target.format
            )),
            false => Ok(()),
        }
    }

    pub(crate) fn shader(&self, wgsl: &str) -> Result<(), String> {
        for extension in enabled(wgsl) {
            let needed = match extension {
                "f16" => wgpu::Features::SHADER_F16,
                "clip_distances" => wgpu::Features::CLIP_DISTANCES,
                "dual_source_blending" => wgpu::Features::DUAL_SOURCE_BLENDING,
                "primitive_index" => wgpu::Features::PRIMITIVE_INDEX,
                _ => continue,
            };
            self.needs(needed)?;
        }
        Ok(())
    }
}

pub(crate) fn enabled(wgsl: &str) -> Vec<&str> {
    let mut extensions = Vec::new();
    let mut rest = wgsl;
    loop {
        rest = skip_blank(rest);
        let word_end = rest
            .find(|character: char| !(character.is_alphanumeric() || character == '_'))
            .unwrap_or(rest.len());
        let (word, after) = rest.split_at(word_end);
        if !matches!(word, "enable" | "requires" | "diagnostic") {
            return extensions;
        }
        let Some(end) = after.find(';') else {
            return extensions;
        };
        if word == "enable" {
            extensions.extend(
                after[..end]
                    .split(',')
                    .map(|name| skip_blank(name).trim_end())
                    .filter(|name| !name.is_empty()),
            );
        }
        rest = &after[end + 1..];
    }
}

fn skip_blank(mut text: &str) -> &str {
    loop {
        text = text.trim_start();
        if let Some(line) = text.strip_prefix("//") {
            text = line.find('\n').map_or("", |end| &line[end..]);
        } else if text.starts_with("/*") {
            let mut depth = 0usize;
            let mut index = 0;
            let bytes = text.as_bytes();
            while index < bytes.len() {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    index += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    index += 1;
                }
            }
            text = &text[index..];
        } else {
            return text;
        }
    }
}
