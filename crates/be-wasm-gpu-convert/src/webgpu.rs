//! Conversions between wasi:webgpu's types and wgpu's.

use be_wasm_gpu_abi::webgpu::webgpu as web;

use super::{Convert, TryConvert, astc};

plain_map!(web::GpuTextureFormat, wgpu::TextureFormat, {
    Stencil8,
    Bc1RgbaUnorm, Bc1RgbaUnormSrgb, Bc2RgbaUnorm, Bc2RgbaUnormSrgb, Bc3RgbaUnorm, Bc3RgbaUnormSrgb,
    Bc4RUnorm, Bc4RSnorm, Bc5RgUnorm, Bc5RgSnorm, Bc6hRgbUfloat, Bc6hRgbFloat,
    Bc7RgbaUnorm, Bc7RgbaUnormSrgb,
}, {
    R8unorm = wgpu::TextureFormat::R8Unorm,
    R8snorm = wgpu::TextureFormat::R8Snorm,
    R8uint = wgpu::TextureFormat::R8Uint,
    R8sint = wgpu::TextureFormat::R8Sint,
    R16unorm = wgpu::TextureFormat::R16Unorm,
    R16snorm = wgpu::TextureFormat::R16Snorm,
    R16uint = wgpu::TextureFormat::R16Uint,
    R16sint = wgpu::TextureFormat::R16Sint,
    R16float = wgpu::TextureFormat::R16Float,
    Rg8unorm = wgpu::TextureFormat::Rg8Unorm,
    Rg8snorm = wgpu::TextureFormat::Rg8Snorm,
    Rg8uint = wgpu::TextureFormat::Rg8Uint,
    Rg8sint = wgpu::TextureFormat::Rg8Sint,
    R32uint = wgpu::TextureFormat::R32Uint,
    R32sint = wgpu::TextureFormat::R32Sint,
    R32float = wgpu::TextureFormat::R32Float,
    Rg16unorm = wgpu::TextureFormat::Rg16Unorm,
    Rg16snorm = wgpu::TextureFormat::Rg16Snorm,
    Rg16uint = wgpu::TextureFormat::Rg16Uint,
    Rg16sint = wgpu::TextureFormat::Rg16Sint,
    Rg16float = wgpu::TextureFormat::Rg16Float,
    Rgba8unorm = wgpu::TextureFormat::Rgba8Unorm,
    Rgba8unormSrgb = wgpu::TextureFormat::Rgba8UnormSrgb,
    Rgba8snorm = wgpu::TextureFormat::Rgba8Snorm,
    Rgba8uint = wgpu::TextureFormat::Rgba8Uint,
    Rgba8sint = wgpu::TextureFormat::Rgba8Sint,
    Bgra8unorm = wgpu::TextureFormat::Bgra8Unorm,
    Bgra8unormSrgb = wgpu::TextureFormat::Bgra8UnormSrgb,
    Rgb9e5ufloat = wgpu::TextureFormat::Rgb9e5Ufloat,
    Rgb10a2uint = wgpu::TextureFormat::Rgb10a2Uint,
    Rgb10a2unorm = wgpu::TextureFormat::Rgb10a2Unorm,
    Rg11b10ufloat = wgpu::TextureFormat::Rg11b10Ufloat,
    Rg32uint = wgpu::TextureFormat::Rg32Uint,
    Rg32sint = wgpu::TextureFormat::Rg32Sint,
    Rg32float = wgpu::TextureFormat::Rg32Float,
    Rgba16unorm = wgpu::TextureFormat::Rgba16Unorm,
    Rgba16snorm = wgpu::TextureFormat::Rgba16Snorm,
    Rgba16uint = wgpu::TextureFormat::Rgba16Uint,
    Rgba16sint = wgpu::TextureFormat::Rgba16Sint,
    Rgba16float = wgpu::TextureFormat::Rgba16Float,
    Rgba32uint = wgpu::TextureFormat::Rgba32Uint,
    Rgba32sint = wgpu::TextureFormat::Rgba32Sint,
    Rgba32float = wgpu::TextureFormat::Rgba32Float,
    Depth16unorm = wgpu::TextureFormat::Depth16Unorm,
    Depth24plus = wgpu::TextureFormat::Depth24Plus,
    Depth24plusStencil8 = wgpu::TextureFormat::Depth24PlusStencil8,
    Depth32float = wgpu::TextureFormat::Depth32Float,
    Depth32floatStencil8 = wgpu::TextureFormat::Depth32FloatStencil8,
    Etc2Rgb8unorm = wgpu::TextureFormat::Etc2Rgb8Unorm,
    Etc2Rgb8unormSrgb = wgpu::TextureFormat::Etc2Rgb8UnormSrgb,
    Etc2Rgb8a1unorm = wgpu::TextureFormat::Etc2Rgb8A1Unorm,
    Etc2Rgb8a1unormSrgb = wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb,
    Etc2Rgba8unorm = wgpu::TextureFormat::Etc2Rgba8Unorm,
    Etc2Rgba8unormSrgb = wgpu::TextureFormat::Etc2Rgba8UnormSrgb,
    EacR11unorm = wgpu::TextureFormat::EacR11Unorm,
    EacR11snorm = wgpu::TextureFormat::EacR11Snorm,
    EacRg11unorm = wgpu::TextureFormat::EacRg11Unorm,
    EacRg11snorm = wgpu::TextureFormat::EacRg11Snorm,
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

plain_map!(web::GpuTextureDimension, wgpu::TextureDimension, { D1, D2, D3 });

plain_map!(web::GpuTextureViewDimension, wgpu::TextureViewDimension, {
    D1, D2, D2Array, Cube, CubeArray, D3,
});

plain_map!(web::GpuTextureAspect, wgpu::TextureAspect, { All, StencilOnly, DepthOnly });

plain_map!(web::GpuAddressMode, wgpu::AddressMode, { ClampToEdge, Repeat, MirrorRepeat });

plain_map!(web::GpuFilterMode, wgpu::FilterMode, { Nearest, Linear });

plain_map!(web::GpuMipmapFilterMode, wgpu::MipmapFilterMode, { Nearest, Linear });

plain_map!(web::GpuCompareFunction, wgpu::CompareFunction, {
    Never, Less, Equal, LessEqual, Greater, NotEqual, GreaterEqual, Always,
});

plain_map!(web::GpuVertexFormat, wgpu::VertexFormat, {
    Uint8, Uint8x2, Uint8x4, Sint8, Sint8x2, Sint8x4,
    Unorm8, Unorm8x2, Unorm8x4, Snorm8, Snorm8x2, Snorm8x4,
    Uint16, Uint16x2, Uint16x4, Sint16, Sint16x2, Sint16x4,
    Unorm16, Unorm16x2, Unorm16x4, Snorm16, Snorm16x2, Snorm16x4,
    Float16, Float16x2, Float16x4,
    Float32, Float32x2, Float32x3, Float32x4,
    Uint32, Uint32x2, Uint32x3, Uint32x4,
    Sint32, Sint32x2, Sint32x3, Sint32x4,
    Unorm8x4Bgra,
}, {
    Unorm1010102 = wgpu::VertexFormat::Unorm10_10_10_2,
});

plain_map!(web::GpuVertexStepMode, wgpu::VertexStepMode, { Vertex, Instance });

plain_map!(web::GpuPrimitiveTopology, wgpu::PrimitiveTopology, {
    PointList, LineList, LineStrip, TriangleList, TriangleStrip,
});

plain_map!(web::GpuIndexFormat, wgpu::IndexFormat, { Uint16, Uint32 });

plain_map!(web::GpuFrontFace, wgpu::FrontFace, { Ccw, Cw });

plain_map!(web::GpuBlendFactor, wgpu::BlendFactor, {
    Zero, One, Src, OneMinusSrc, SrcAlpha, OneMinusSrcAlpha,
    Dst, OneMinusDst, DstAlpha, OneMinusDstAlpha,
    SrcAlphaSaturated, Constant, OneMinusConstant,
    Src1, OneMinusSrc1, Src1Alpha, OneMinusSrc1Alpha,
});

plain_map!(web::GpuBlendOperation, wgpu::BlendOperation, {
    Add, Subtract, ReverseSubtract, Min, Max,
});

plain_map!(web::GpuStencilOperation, wgpu::StencilOperation, {
    Keep, Zero, Replace, Invert,
    IncrementClamp, DecrementClamp, IncrementWrap, DecrementWrap,
});

plain_map!(web::GpuSamplerBindingType, wgpu::SamplerBindingType, {
    Filtering, NonFiltering, Comparison,
});

plain_map!(web::GpuStorageTextureAccess, wgpu::StorageTextureAccess, {
    WriteOnly, ReadOnly, ReadWrite,
});

plain_map!(web::GpuStoreOp, wgpu::StoreOp, { Store, Discard });

plain_map!(web::GpuQueryType, wgpu::QueryType, { Occlusion, Timestamp });

/// The wgpu feature a WebGPU feature is, if wgpu has it: `core-features-and-limits` is none
/// (every device is core), and wgpu 30 has no `texture-formats-tier1`, `texture-formats-tier2`
/// or `texture-component-swizzle`, so no host offers them.
pub fn feature(name: web::GpuFeatureName) -> Option<wgpu::Features> {
    use web::GpuFeatureName as F;
    Some(match name {
        F::CoreFeaturesAndLimits => wgpu::Features::empty(),
        F::DepthClipControl => wgpu::Features::DEPTH_CLIP_CONTROL,
        F::Depth32floatStencil8 => wgpu::Features::DEPTH32FLOAT_STENCIL8,
        F::TextureCompressionBc => wgpu::Features::TEXTURE_COMPRESSION_BC,
        F::TextureCompressionBcSliced3d => wgpu::Features::TEXTURE_COMPRESSION_BC_SLICED_3D,
        F::TextureCompressionEtc2 => wgpu::Features::TEXTURE_COMPRESSION_ETC2,
        F::TextureCompressionAstc => wgpu::Features::TEXTURE_COMPRESSION_ASTC,
        F::TextureCompressionAstcSliced3d => wgpu::Features::TEXTURE_COMPRESSION_ASTC_SLICED_3D,
        F::TimestampQuery => wgpu::Features::TIMESTAMP_QUERY,
        F::IndirectFirstInstance => wgpu::Features::INDIRECT_FIRST_INSTANCE,
        F::ShaderF16 => wgpu::Features::SHADER_F16,
        F::Rg11b10ufloatRenderable => wgpu::Features::RG11B10UFLOAT_RENDERABLE,
        F::Bgra8unormStorage => wgpu::Features::BGRA8UNORM_STORAGE,
        F::Float32Filterable => wgpu::Features::FLOAT32_FILTERABLE,
        F::Float32Blendable => wgpu::Features::FLOAT32_BLENDABLE,
        F::ClipDistances => wgpu::Features::CLIP_DISTANCES,
        F::DualSourceBlending => wgpu::Features::DUAL_SOURCE_BLENDING,
        F::Subgroups => wgpu::Features::SUBGROUP,
        F::PrimitiveIndex => wgpu::Features::PRIMITIVE_INDEX,
        F::TextureFormatsTier1 | F::TextureFormatsTier2 | F::TextureComponentSwizzle => {
            return None;
        }
    })
}

/// The WebGPU features a wgpu device's features make.
pub fn features(features: wgpu::Features) -> Vec<web::GpuFeatureName> {
    FEATURES
        .into_iter()
        .filter(|name| feature(*name).is_some_and(|wanted| features.contains(wanted)))
        .collect()
}

/// The WebGPU name of a feature (`GPUFeatureName`), as `GPUSupportedFeatures.has` takes it.
pub fn feature_name(feature: web::GpuFeatureName) -> &'static str {
    use web::GpuFeatureName as F;
    match feature {
        F::CoreFeaturesAndLimits => "core-features-and-limits",
        F::DepthClipControl => "depth-clip-control",
        F::Depth32floatStencil8 => "depth32float-stencil8",
        F::TextureCompressionBc => "texture-compression-bc",
        F::TextureCompressionBcSliced3d => "texture-compression-bc-sliced-3d",
        F::TextureCompressionEtc2 => "texture-compression-etc2",
        F::TextureCompressionAstc => "texture-compression-astc",
        F::TextureCompressionAstcSliced3d => "texture-compression-astc-sliced-3d",
        F::TimestampQuery => "timestamp-query",
        F::IndirectFirstInstance => "indirect-first-instance",
        F::ShaderF16 => "shader-f16",
        F::Rg11b10ufloatRenderable => "rg11b10ufloat-renderable",
        F::Bgra8unormStorage => "bgra8unorm-storage",
        F::Float32Filterable => "float32-filterable",
        F::Float32Blendable => "float32-blendable",
        F::ClipDistances => "clip-distances",
        F::DualSourceBlending => "dual-source-blending",
        F::Subgroups => "subgroups",
        F::TextureFormatsTier1 => "texture-formats-tier1",
        F::TextureFormatsTier2 => "texture-formats-tier2",
        F::PrimitiveIndex => "primitive-index",
        F::TextureComponentSwizzle => "texture-component-swizzle",
    }
}

/// Every feature wasi:webgpu names, in its order.
pub const FEATURES: [web::GpuFeatureName; 22] = {
    use web::GpuFeatureName as F;
    [
        F::CoreFeaturesAndLimits,
        F::DepthClipControl,
        F::Depth32floatStencil8,
        F::TextureCompressionBc,
        F::TextureCompressionBcSliced3d,
        F::TextureCompressionEtc2,
        F::TextureCompressionAstc,
        F::TextureCompressionAstcSliced3d,
        F::TimestampQuery,
        F::IndirectFirstInstance,
        F::ShaderF16,
        F::Rg11b10ufloatRenderable,
        F::Bgra8unormStorage,
        F::Float32Filterable,
        F::Float32Blendable,
        F::ClipDistances,
        F::DualSourceBlending,
        F::Subgroups,
        F::TextureFormatsTier1,
        F::TextureFormatsTier2,
        F::PrimitiveIndex,
        F::TextureComponentSwizzle,
    ]
};

impl Convert<wgpu::Color> for web::GpuColor {
    fn convert(self) -> wgpu::Color {
        wgpu::Color {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
    }
}

impl Convert<wgpu::Extent3d> for web::GpuExtent3D {
    fn convert(self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.width,
            height: self.height.unwrap_or(1),
            depth_or_array_layers: self.depth_or_array_layers.unwrap_or(1),
        }
    }
}

impl Convert<wgpu::Origin3d> for Option<web::GpuOrigin3D> {
    fn convert(self) -> wgpu::Origin3d {
        let origin = self.unwrap_or(web::GpuOrigin3D {
            x: None,
            y: None,
            z: None,
        });
        wgpu::Origin3d {
            x: origin.x.unwrap_or(0),
            y: origin.y.unwrap_or(0),
            z: origin.z.unwrap_or(0),
        }
    }
}

impl Convert<wgpu::BlendComponent> for web::GpuBlendComponent {
    fn convert(self) -> wgpu::BlendComponent {
        wgpu::BlendComponent {
            src_factor: self
                .src_factor
                .map_or(wgpu::BlendFactor::One, Convert::convert),
            dst_factor: self
                .dst_factor
                .map_or(wgpu::BlendFactor::Zero, Convert::convert),
            operation: self
                .operation
                .map_or(wgpu::BlendOperation::Add, Convert::convert),
        }
    }
}

impl Convert<wgpu::StencilFaceState> for Option<web::GpuStencilFaceState> {
    fn convert(self) -> wgpu::StencilFaceState {
        let Some(state) = self else {
            return wgpu::StencilFaceState::IGNORE;
        };
        wgpu::StencilFaceState {
            compare: state
                .compare
                .map_or(wgpu::CompareFunction::Always, Convert::convert),
            fail_op: state
                .fail_op
                .map_or(wgpu::StencilOperation::Keep, Convert::convert),
            depth_fail_op: state
                .depth_fail_op
                .map_or(wgpu::StencilOperation::Keep, Convert::convert),
            pass_op: state
                .pass_op
                .map_or(wgpu::StencilOperation::Keep, Convert::convert),
        }
    }
}

/// A color write mask: wasi:webgpu's `all` is a flag of its own, where WebGPU's `ALL` is the
/// other four together.
impl Convert<wgpu::ColorWrites> for web::GpuColorWrite {
    fn convert(self) -> wgpu::ColorWrites {
        match self.contains(web::GpuColorWrite::ALL) {
            true => wgpu::ColorWrites::ALL,
            false => wgpu::ColorWrites::from_bits_truncate(self.bits()),
        }
    }
}

impl Convert<wgpu::BufferUsages> for web::GpuBufferUsage {
    fn convert(self) -> wgpu::BufferUsages {
        wgpu::BufferUsages::from_bits_truncate(self.bits() & 0x3ff)
    }
}

impl Convert<web::GpuBufferUsage> for wgpu::BufferUsages {
    fn convert(self) -> web::GpuBufferUsage {
        web::GpuBufferUsage::from_bits(self.bits() & 0x3ff)
    }
}

impl Convert<wgpu::TextureUsages> for web::GpuTextureUsage {
    fn convert(self) -> wgpu::TextureUsages {
        wgpu::TextureUsages::from_bits_truncate(self.bits() & 0x3f)
    }
}

impl Convert<web::GpuTextureUsage> for wgpu::TextureUsages {
    fn convert(self) -> web::GpuTextureUsage {
        web::GpuTextureUsage::from_bits(self.bits() & 0x3f)
    }
}

impl Convert<wgpu::ShaderStages> for web::GpuShaderStage {
    fn convert(self) -> wgpu::ShaderStages {
        wgpu::ShaderStages::from_bits_truncate(self.bits() & 0x7)
    }
}

impl Convert<wgpu::BindingType> for &web::GpuBindGroupLayoutEntry {
    fn convert(self) -> wgpu::BindingType {
        if let Some(buffer) = &self.buffer {
            return wgpu::BindingType::Buffer {
                ty: match buffer.r#type.unwrap_or(web::GpuBufferBindingType::Uniform) {
                    web::GpuBufferBindingType::Uniform => wgpu::BufferBindingType::Uniform,
                    web::GpuBufferBindingType::Storage => {
                        wgpu::BufferBindingType::Storage { read_only: false }
                    }
                    web::GpuBufferBindingType::ReadOnlyStorage => {
                        wgpu::BufferBindingType::Storage { read_only: true }
                    }
                },
                has_dynamic_offset: buffer.has_dynamic_offset.unwrap_or(false),
                min_binding_size: buffer.min_binding_size.and_then(wgpu::BufferSize::new),
            };
        }
        if let Some(sampler) = &self.sampler {
            return wgpu::BindingType::Sampler(
                sampler
                    .r#type
                    .map_or(wgpu::SamplerBindingType::Filtering, Convert::convert),
            );
        }
        if let Some(storage) = &self.storage_texture {
            return wgpu::BindingType::StorageTexture {
                access: storage
                    .access
                    .map_or(wgpu::StorageTextureAccess::WriteOnly, Convert::convert),
                format: storage.format.convert(),
                view_dimension: storage
                    .view_dimension
                    .map_or(wgpu::TextureViewDimension::D2, Convert::convert),
            };
        }
        let texture = self.texture.unwrap_or(web::GpuTextureBindingLayout {
            sample_type: None,
            view_dimension: None,
            multisampled: None,
        });
        wgpu::BindingType::Texture {
            sample_type: match texture
                .sample_type
                .unwrap_or(web::GpuTextureSampleType::Float)
            {
                web::GpuTextureSampleType::Float => {
                    wgpu::TextureSampleType::Float { filterable: true }
                }
                web::GpuTextureSampleType::UnfilterableFloat => {
                    wgpu::TextureSampleType::Float { filterable: false }
                }
                web::GpuTextureSampleType::Depth => wgpu::TextureSampleType::Depth,
                web::GpuTextureSampleType::Sint => wgpu::TextureSampleType::Sint,
                web::GpuTextureSampleType::Uint => wgpu::TextureSampleType::Uint,
            },
            view_dimension: texture
                .view_dimension
                .map_or(wgpu::TextureViewDimension::D2, Convert::convert),
            multisampled: texture.multisampled.unwrap_or(false),
        }
    }
}

/// How many of a bind group layout entry's binding kinds it names: WebGPU asks for exactly one.
pub fn binding_kinds(entry: &web::GpuBindGroupLayoutEntry) -> usize {
    [
        entry.buffer.is_some(),
        entry.sampler.is_some(),
        entry.texture.is_some(),
        entry.storage_texture.is_some(),
    ]
    .into_iter()
    .filter(|named| *named)
    .count()
}

impl Convert<web::GpuCompilationMessageType> for wgpu::CompilationMessageType {
    fn convert(self) -> web::GpuCompilationMessageType {
        match self {
            wgpu::CompilationMessageType::Error => web::GpuCompilationMessageType::Error,
            wgpu::CompilationMessageType::Warning => web::GpuCompilationMessageType::Warning,
            wgpu::CompilationMessageType::Info => web::GpuCompilationMessageType::Info,
        }
    }
}
