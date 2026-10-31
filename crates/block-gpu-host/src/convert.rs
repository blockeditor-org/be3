use block_gpu_abi::device as abi;

macro_rules! plain_map {
    ($function:ident, $from:path, $to:path, { $($variant:ident),+ $(,)? } $(, { $($named:ident = $value:expr),+ $(,)? })?) => {
        pub fn $function(value: $from) -> $to {
            use $from as From;
            use $to as To;
            match value {
                $(From::$variant => To::$variant,)+
                $($(From::$named => $value,)+)?
            }
        }
    };
}

const fn astc(block: wgpu::AstcBlock, srgb: bool) -> wgpu::TextureFormat {
    wgpu::TextureFormat::Astc {
        block,
        channel: match srgb {
            true => wgpu::AstcChannel::UnormSrgb,
            false => wgpu::AstcChannel::Unorm,
        },
    }
}

plain_map!(texture_format, abi::TextureFormat, wgpu::TextureFormat, {
    R8Unorm, R8Snorm, R8Uint, R8Sint,
    R16Uint, R16Sint, R16Float,
    Rg8Unorm, Rg8Snorm, Rg8Uint, Rg8Sint,
    R32Uint, R32Sint, R32Float,
    Rg16Uint, Rg16Sint, Rg16Float,
    Rgba8Unorm, Rgba8UnormSrgb, Rgba8Snorm, Rgba8Uint, Rgba8Sint,
    Bgra8Unorm, Bgra8UnormSrgb,
    Rgb9e5Ufloat, Rgb10a2Uint, Rgb10a2Unorm, Rg11b10Ufloat,
    Rg32Uint, Rg32Sint, Rg32Float,
    Rgba16Uint, Rgba16Sint, Rgba16Float,
    Rgba32Uint, Rgba32Sint, Rgba32Float,
    Stencil8, Depth16Unorm, Depth24Plus, Depth24PlusStencil8, Depth32Float, Depth32FloatStencil8,
    Bc1RgbaUnorm, Bc1RgbaUnormSrgb, Bc2RgbaUnorm, Bc2RgbaUnormSrgb, Bc3RgbaUnorm, Bc3RgbaUnormSrgb,
    Bc4RUnorm, Bc4RSnorm, Bc5RgUnorm, Bc5RgSnorm, Bc6hRgbUfloat, Bc6hRgbFloat,
    Bc7RgbaUnorm, Bc7RgbaUnormSrgb,
    Etc2Rgb8Unorm, Etc2Rgb8UnormSrgb, Etc2Rgba8Unorm, Etc2Rgba8UnormSrgb,
    EacR11Unorm, EacR11Snorm, EacRg11Unorm, EacRg11Snorm,
}, {
    Etc2Rgb8a1Unorm = wgpu::TextureFormat::Etc2Rgb8A1Unorm,
    Etc2Rgb8a1UnormSrgb = wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb,
    Astc4x4Unorm = astc(wgpu::AstcBlock::B4x4, false),
    Astc4x4UnormSrgb = astc(wgpu::AstcBlock::B4x4, true),
    Astc5x4Unorm = astc(wgpu::AstcBlock::B5x4, false),
    Astc5x4UnormSrgb = astc(wgpu::AstcBlock::B5x4, true),
    Astc5x5Unorm = astc(wgpu::AstcBlock::B5x5, false),
    Astc5x5UnormSrgb = astc(wgpu::AstcBlock::B5x5, true),
    Astc6x5Unorm = astc(wgpu::AstcBlock::B6x5, false),
    Astc6x5UnormSrgb = astc(wgpu::AstcBlock::B6x5, true),
    Astc6x6Unorm = astc(wgpu::AstcBlock::B6x6, false),
    Astc6x6UnormSrgb = astc(wgpu::AstcBlock::B6x6, true),
    Astc8x5Unorm = astc(wgpu::AstcBlock::B8x5, false),
    Astc8x5UnormSrgb = astc(wgpu::AstcBlock::B8x5, true),
    Astc8x6Unorm = astc(wgpu::AstcBlock::B8x6, false),
    Astc8x6UnormSrgb = astc(wgpu::AstcBlock::B8x6, true),
    Astc8x8Unorm = astc(wgpu::AstcBlock::B8x8, false),
    Astc8x8UnormSrgb = astc(wgpu::AstcBlock::B8x8, true),
    Astc10x5Unorm = astc(wgpu::AstcBlock::B10x5, false),
    Astc10x5UnormSrgb = astc(wgpu::AstcBlock::B10x5, true),
    Astc10x6Unorm = astc(wgpu::AstcBlock::B10x6, false),
    Astc10x6UnormSrgb = astc(wgpu::AstcBlock::B10x6, true),
    Astc10x8Unorm = astc(wgpu::AstcBlock::B10x8, false),
    Astc10x8UnormSrgb = astc(wgpu::AstcBlock::B10x8, true),
    Astc10x10Unorm = astc(wgpu::AstcBlock::B10x10, false),
    Astc10x10UnormSrgb = astc(wgpu::AstcBlock::B10x10, true),
    Astc12x10Unorm = astc(wgpu::AstcBlock::B12x10, false),
    Astc12x10UnormSrgb = astc(wgpu::AstcBlock::B12x10, true),
    Astc12x12Unorm = astc(wgpu::AstcBlock::B12x12, false),
    Astc12x12UnormSrgb = astc(wgpu::AstcBlock::B12x12, true),
});

plain_map!(texture_dimension, abi::TextureDimension, wgpu::TextureDimension, {
    D1, D2, D3,
});

plain_map!(texture_view_dimension, abi::TextureViewDimension, wgpu::TextureViewDimension, {
    D1, D2, D2Array, Cube, CubeArray, D3,
});

plain_map!(texture_aspect, abi::TextureAspect, wgpu::TextureAspect, {
    All, StencilOnly, DepthOnly,
});

plain_map!(address_mode, abi::AddressMode, wgpu::AddressMode, {
    ClampToEdge, Repeat, MirrorRepeat, ClampToBorder,
});

plain_map!(filter_mode, abi::FilterMode, wgpu::FilterMode, { Nearest, Linear });

plain_map!(mipmap_filter_mode, abi::FilterMode, wgpu::MipmapFilterMode, { Nearest, Linear });

plain_map!(compare_function, abi::CompareFunction, wgpu::CompareFunction, {
    Never, Less, Equal, LessEqual, Greater, NotEqual, GreaterEqual, Always,
});

plain_map!(border_color, abi::SamplerBorderColor, wgpu::SamplerBorderColor, {
    TransparentBlack, OpaqueBlack, OpaqueWhite, Zero,
});

plain_map!(vertex_format, abi::VertexFormat, wgpu::VertexFormat, {
    Uint8x2, Uint8x4, Sint8x2, Sint8x4,
    Unorm8x2, Unorm8x4, Snorm8x2, Snorm8x4,
    Uint16x2, Uint16x4, Sint16x2, Sint16x4,
    Unorm16x2, Unorm16x4, Snorm16x2, Snorm16x4,
    Float16x2, Float16x4,
    Float32, Float32x2, Float32x3, Float32x4,
    Uint32, Uint32x2, Uint32x3, Uint32x4,
    Sint32, Sint32x2, Sint32x3, Sint32x4,
});

plain_map!(step_mode, abi::VertexStepMode, wgpu::VertexStepMode, { Vertex, Instance });

plain_map!(topology, abi::PrimitiveTopology, wgpu::PrimitiveTopology, {
    PointList, LineList, LineStrip, TriangleList, TriangleStrip,
});

plain_map!(index_format, abi::IndexFormat, wgpu::IndexFormat, { Uint16, Uint32 });

plain_map!(front_face, abi::FrontFace, wgpu::FrontFace, { Ccw, Cw });

plain_map!(face, abi::Face, wgpu::Face, { Front, Back });

plain_map!(polygon_mode, abi::PolygonMode, wgpu::PolygonMode, { Fill, Line, Point });

plain_map!(blend_factor, abi::BlendFactor, wgpu::BlendFactor, {
    Zero, One, Src, OneMinusSrc, SrcAlpha, OneMinusSrcAlpha,
    Dst, OneMinusDst, DstAlpha, OneMinusDstAlpha,
    SrcAlphaSaturated, Constant, OneMinusConstant,
    Src1, OneMinusSrc1, Src1Alpha, OneMinusSrc1Alpha,
});

plain_map!(blend_operation, abi::BlendOperation, wgpu::BlendOperation, {
    Add, Subtract, ReverseSubtract, Min, Max,
});

plain_map!(stencil_operation, abi::StencilOperation, wgpu::StencilOperation, {
    Keep, Zero, Replace, Invert,
    IncrementClamp, DecrementClamp, IncrementWrap, DecrementWrap,
});

plain_map!(sampler_binding, abi::SamplerBindingType, wgpu::SamplerBindingType, {
    Filtering, NonFiltering, Comparison,
});

plain_map!(storage_access, abi::StorageTextureAccess, wgpu::StorageTextureAccess, {
    WriteOnly, ReadOnly, ReadWrite,
});

plain_map!(store_op, abi::StoreOp, wgpu::StoreOp, { Store, Discard });

pub(crate) fn constants(values: &[(String, f64)]) -> Vec<(&str, f64)> {
    values
        .iter()
        .map(|(key, value)| (key.as_str(), *value))
        .collect()
}

pub(crate) fn label(value: &Option<String>) -> Option<&str> {
    value.as_deref()
}

pub(crate) fn extent(value: abi::Extent3d) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: value.width,
        height: value.height,
        depth_or_array_layers: value.depth_or_array_layers,
    }
}

pub(crate) fn size(value: Option<u64>) -> Option<wgpu::BufferSize> {
    value.and_then(wgpu::BufferSize::new)
}

pub(crate) fn blend_component(value: abi::BlendComponent) -> wgpu::BlendComponent {
    wgpu::BlendComponent {
        src_factor: blend_factor(value.src_factor),
        dst_factor: blend_factor(value.dst_factor),
        operation: blend_operation(value.operation),
    }
}

pub(crate) fn stencil_face(value: abi::StencilFaceState) -> wgpu::StencilFaceState {
    wgpu::StencilFaceState {
        compare: compare_function(value.compare),
        fail_op: stencil_operation(value.fail_op),
        depth_fail_op: stencil_operation(value.depth_fail_op),
        pass_op: stencil_operation(value.pass_op),
    }
}

pub(crate) fn color(value: abi::Color) -> wgpu::Color {
    wgpu::Color {
        r: value.r,
        g: value.g,
        b: value.b,
        a: value.a,
    }
}

pub(crate) fn binding_type(value: abi::BindingType) -> wgpu::BindingType {
    match value {
        abi::BindingType::Buffer(abi::BufferBindingLayout {
            kind,
            has_dynamic_offset,
            min_binding_size,
        }) => wgpu::BindingType::Buffer {
            ty: match kind {
                abi::BufferBindingType::Uniform => wgpu::BufferBindingType::Uniform,
                abi::BufferBindingType::Storage => {
                    wgpu::BufferBindingType::Storage { read_only: false }
                }
                abi::BufferBindingType::ReadOnlyStorage => {
                    wgpu::BufferBindingType::Storage { read_only: true }
                }
            },
            has_dynamic_offset,
            min_binding_size: size(min_binding_size),
        },
        abi::BindingType::Sampler(kind) => wgpu::BindingType::Sampler(sampler_binding(kind)),
        abi::BindingType::Texture(abi::TextureBindingLayout {
            sample_type,
            view_dimension,
            multisampled,
        }) => wgpu::BindingType::Texture {
            sample_type: match sample_type {
                abi::TextureSampleType::FloatFilterable => {
                    wgpu::TextureSampleType::Float { filterable: true }
                }
                abi::TextureSampleType::FloatUnfilterable => {
                    wgpu::TextureSampleType::Float { filterable: false }
                }
                abi::TextureSampleType::Depth => wgpu::TextureSampleType::Depth,
                abi::TextureSampleType::Sint => wgpu::TextureSampleType::Sint,
                abi::TextureSampleType::Uint => wgpu::TextureSampleType::Uint,
            },
            view_dimension: texture_view_dimension(view_dimension),
            multisampled,
        },
        abi::BindingType::StorageTexture(abi::StorageTextureBindingLayout {
            access,
            format,
            view_dimension,
        }) => wgpu::BindingType::StorageTexture {
            access: storage_access(access),
            format: texture_format(format),
            view_dimension: texture_view_dimension(view_dimension),
        },
    }
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

macro_rules! reverse_map {
    ($function:ident, $from:path, $to:path, { $($variant:ident),+ $(,)? } $(, { $($named:ident = $value:expr),+ $(,)? })?) => {
        pub(crate) fn $function(value: $from) -> Option<$to> {
            use $from as From;
            use $to as To;
            #[allow(unreachable_patterns)]
            match value {
                $(From::$variant => Some(To::$variant),)+
                other => {
                    $($(if other == $value {
                        return Some(To::$named);
                    })+)?
                    let _ = other;
                    None
                }
            }
        }
    };
}

reverse_map!(abi_texture_format, wgpu::TextureFormat, abi::TextureFormat, {
    R8Unorm, R8Snorm, R8Uint, R8Sint,
    R16Uint, R16Sint, R16Float,
    Rg8Unorm, Rg8Snorm, Rg8Uint, Rg8Sint,
    R32Uint, R32Sint, R32Float,
    Rg16Uint, Rg16Sint, Rg16Float,
    Rgba8Unorm, Rgba8UnormSrgb, Rgba8Snorm, Rgba8Uint, Rgba8Sint,
    Bgra8Unorm, Bgra8UnormSrgb,
    Rgb9e5Ufloat, Rgb10a2Uint, Rgb10a2Unorm, Rg11b10Ufloat,
    Rg32Uint, Rg32Sint, Rg32Float,
    Rgba16Uint, Rgba16Sint, Rgba16Float,
    Rgba32Uint, Rgba32Sint, Rgba32Float,
    Stencil8, Depth16Unorm, Depth24Plus, Depth24PlusStencil8, Depth32Float, Depth32FloatStencil8,
    Bc1RgbaUnorm, Bc1RgbaUnormSrgb, Bc2RgbaUnorm, Bc2RgbaUnormSrgb, Bc3RgbaUnorm, Bc3RgbaUnormSrgb,
    Bc4RUnorm, Bc4RSnorm, Bc5RgUnorm, Bc5RgSnorm, Bc6hRgbUfloat, Bc6hRgbFloat,
    Bc7RgbaUnorm, Bc7RgbaUnormSrgb,
    Etc2Rgb8Unorm, Etc2Rgb8UnormSrgb, Etc2Rgba8Unorm, Etc2Rgba8UnormSrgb,
    EacR11Unorm, EacR11Snorm, EacRg11Unorm, EacRg11Snorm,
}, {
    Etc2Rgb8a1Unorm = wgpu::TextureFormat::Etc2Rgb8A1Unorm,
    Etc2Rgb8a1UnormSrgb = wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb,
    Astc4x4Unorm = astc(wgpu::AstcBlock::B4x4, false),
    Astc4x4UnormSrgb = astc(wgpu::AstcBlock::B4x4, true),
    Astc5x4Unorm = astc(wgpu::AstcBlock::B5x4, false),
    Astc5x4UnormSrgb = astc(wgpu::AstcBlock::B5x4, true),
    Astc5x5Unorm = astc(wgpu::AstcBlock::B5x5, false),
    Astc5x5UnormSrgb = astc(wgpu::AstcBlock::B5x5, true),
    Astc6x5Unorm = astc(wgpu::AstcBlock::B6x5, false),
    Astc6x5UnormSrgb = astc(wgpu::AstcBlock::B6x5, true),
    Astc6x6Unorm = astc(wgpu::AstcBlock::B6x6, false),
    Astc6x6UnormSrgb = astc(wgpu::AstcBlock::B6x6, true),
    Astc8x5Unorm = astc(wgpu::AstcBlock::B8x5, false),
    Astc8x5UnormSrgb = astc(wgpu::AstcBlock::B8x5, true),
    Astc8x6Unorm = astc(wgpu::AstcBlock::B8x6, false),
    Astc8x6UnormSrgb = astc(wgpu::AstcBlock::B8x6, true),
    Astc8x8Unorm = astc(wgpu::AstcBlock::B8x8, false),
    Astc8x8UnormSrgb = astc(wgpu::AstcBlock::B8x8, true),
    Astc10x5Unorm = astc(wgpu::AstcBlock::B10x5, false),
    Astc10x5UnormSrgb = astc(wgpu::AstcBlock::B10x5, true),
    Astc10x6Unorm = astc(wgpu::AstcBlock::B10x6, false),
    Astc10x6UnormSrgb = astc(wgpu::AstcBlock::B10x6, true),
    Astc10x8Unorm = astc(wgpu::AstcBlock::B10x8, false),
    Astc10x8UnormSrgb = astc(wgpu::AstcBlock::B10x8, true),
    Astc10x10Unorm = astc(wgpu::AstcBlock::B10x10, false),
    Astc10x10UnormSrgb = astc(wgpu::AstcBlock::B10x10, true),
    Astc12x10Unorm = astc(wgpu::AstcBlock::B12x10, false),
    Astc12x10UnormSrgb = astc(wgpu::AstcBlock::B12x10, true),
    Astc12x12Unorm = astc(wgpu::AstcBlock::B12x12, false),
    Astc12x12UnormSrgb = astc(wgpu::AstcBlock::B12x12, true),
});

reverse_map!(abi_texture_dimension, wgpu::TextureDimension, abi::TextureDimension, {
    D1, D2, D3,
});

pub(crate) fn compilation_message(value: &wgpu::CompilationMessage) -> abi::CompilationMessage {
    abi::CompilationMessage {
        message: value.message.clone(),
        message_type: match value.message_type {
            wgpu::CompilationMessageType::Error => abi::CompilationMessageType::Error,
            wgpu::CompilationMessageType::Warning => abi::CompilationMessageType::Warning,
            wgpu::CompilationMessageType::Info => abi::CompilationMessageType::Info,
        },
        location: value.location.map(|location| abi::SourceLocation {
            line_number: location.line_number,
            line_position: location.line_position,
            offset: location.offset,
            length: location.length,
        }),
    }
}
