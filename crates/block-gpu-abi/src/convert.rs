use crate::device as abi;

macro_rules! plain_map {
    ($abi:path, $wgpu:path, { $($variant:ident),+ $(,)? } $(, { $($named:ident = $value:expr),+ $(,)? })?) => {
        impl From<$abi> for $wgpu {
            fn from(value: $abi) -> Self {
                use $abi as Abi;
                use $wgpu as Wgpu;
                match value {
                    $(Abi::$variant => Wgpu::$variant,)+
                    $($(Abi::$named => $value,)+)?
                }
            }
        }

        impl TryFrom<$wgpu> for $abi {
            type Error = $wgpu;

            #[allow(unreachable_patterns)]
            fn try_from(value: $wgpu) -> Result<Self, $wgpu> {
                use $abi as Abi;
                use $wgpu as Wgpu;
                match value {
                    $(Wgpu::$variant => Ok(Abi::$variant),)+
                    other => {
                        $($(if other == $value {
                            return Ok(Abi::$named);
                        })+)?
                        Err(other)
                    }
                }
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

plain_map!(abi::TextureFormat, wgpu::TextureFormat, {
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

plain_map!(abi::TextureDimension, wgpu::TextureDimension, {
    D1, D2, D3,
});

plain_map!(abi::TextureViewDimension, wgpu::TextureViewDimension, {
    D1, D2, D2Array, Cube, CubeArray, D3,
});

plain_map!(abi::TextureAspect, wgpu::TextureAspect, {
    All, StencilOnly, DepthOnly,
});

plain_map!(abi::AddressMode, wgpu::AddressMode, {
    ClampToEdge, Repeat, MirrorRepeat, ClampToBorder,
});

plain_map!(abi::FilterMode, wgpu::FilterMode, { Nearest, Linear });

plain_map!(abi::FilterMode, wgpu::MipmapFilterMode, { Nearest, Linear });

plain_map!(abi::CompareFunction, wgpu::CompareFunction, {
    Never, Less, Equal, LessEqual, Greater, NotEqual, GreaterEqual, Always,
});

plain_map!(abi::SamplerBorderColor, wgpu::SamplerBorderColor, {
    TransparentBlack, OpaqueBlack, OpaqueWhite, Zero,
});

plain_map!(abi::VertexFormat, wgpu::VertexFormat, {
    Uint8x2, Uint8x4, Sint8x2, Sint8x4,
    Unorm8x2, Unorm8x4, Snorm8x2, Snorm8x4,
    Uint16x2, Uint16x4, Sint16x2, Sint16x4,
    Unorm16x2, Unorm16x4, Snorm16x2, Snorm16x4,
    Float16x2, Float16x4,
    Float32, Float32x2, Float32x3, Float32x4,
    Uint32, Uint32x2, Uint32x3, Uint32x4,
    Sint32, Sint32x2, Sint32x3, Sint32x4,
});

plain_map!(abi::VertexStepMode, wgpu::VertexStepMode, { Vertex, Instance });

plain_map!(abi::PrimitiveTopology, wgpu::PrimitiveTopology, {
    PointList, LineList, LineStrip, TriangleList, TriangleStrip,
});

plain_map!(abi::IndexFormat, wgpu::IndexFormat, { Uint16, Uint32 });

plain_map!(abi::FrontFace, wgpu::FrontFace, { Ccw, Cw });

plain_map!(abi::Face, wgpu::Face, { Front, Back });

plain_map!(abi::PolygonMode, wgpu::PolygonMode, { Fill, Line, Point });

plain_map!(abi::BlendFactor, wgpu::BlendFactor, {
    Zero, One, Src, OneMinusSrc, SrcAlpha, OneMinusSrcAlpha,
    Dst, OneMinusDst, DstAlpha, OneMinusDstAlpha,
    SrcAlphaSaturated, Constant, OneMinusConstant,
    Src1, OneMinusSrc1, Src1Alpha, OneMinusSrc1Alpha,
});

plain_map!(abi::BlendOperation, wgpu::BlendOperation, {
    Add, Subtract, ReverseSubtract, Min, Max,
});

plain_map!(abi::StencilOperation, wgpu::StencilOperation, {
    Keep, Zero, Replace, Invert,
    IncrementClamp, DecrementClamp, IncrementWrap, DecrementWrap,
});

plain_map!(abi::SamplerBindingType, wgpu::SamplerBindingType, {
    Filtering, NonFiltering, Comparison,
});

plain_map!(abi::StorageTextureAccess, wgpu::StorageTextureAccess, {
    WriteOnly, ReadOnly, ReadWrite,
});

plain_map!(abi::StoreOp, wgpu::StoreOp, { Store, Discard });

impl From<abi::Extent3d> for wgpu::Extent3d {
    fn from(value: abi::Extent3d) -> Self {
        wgpu::Extent3d {
            width: value.width,
            height: value.height,
            depth_or_array_layers: value.depth_or_array_layers,
        }
    }
}

impl From<wgpu::Extent3d> for abi::Extent3d {
    fn from(value: wgpu::Extent3d) -> Self {
        abi::Extent3d {
            width: value.width,
            height: value.height,
            depth_or_array_layers: value.depth_or_array_layers,
        }
    }
}

impl From<abi::Color> for wgpu::Color {
    fn from(value: abi::Color) -> Self {
        wgpu::Color {
            r: value.r,
            g: value.g,
            b: value.b,
            a: value.a,
        }
    }
}

impl From<wgpu::Color> for abi::Color {
    fn from(value: wgpu::Color) -> Self {
        abi::Color {
            r: value.r,
            g: value.g,
            b: value.b,
            a: value.a,
        }
    }
}

impl From<abi::BlendComponent> for wgpu::BlendComponent {
    fn from(value: abi::BlendComponent) -> Self {
        wgpu::BlendComponent {
            src_factor: value.src_factor.into(),
            dst_factor: value.dst_factor.into(),
            operation: value.operation.into(),
        }
    }
}

impl TryFrom<wgpu::BlendComponent> for abi::BlendComponent {
    type Error = wgpu::BlendComponent;

    fn try_from(value: wgpu::BlendComponent) -> Result<Self, Self::Error> {
        let convert = || {
            Some(abi::BlendComponent {
                src_factor: value.src_factor.try_into().ok()?,
                dst_factor: value.dst_factor.try_into().ok()?,
                operation: value.operation.try_into().ok()?,
            })
        };
        convert().ok_or(value)
    }
}

impl From<abi::StencilFaceState> for wgpu::StencilFaceState {
    fn from(value: abi::StencilFaceState) -> Self {
        wgpu::StencilFaceState {
            compare: value.compare.into(),
            fail_op: value.fail_op.into(),
            depth_fail_op: value.depth_fail_op.into(),
            pass_op: value.pass_op.into(),
        }
    }
}

impl TryFrom<wgpu::StencilFaceState> for abi::StencilFaceState {
    type Error = wgpu::StencilFaceState;

    fn try_from(value: wgpu::StencilFaceState) -> Result<Self, Self::Error> {
        let convert = || {
            Some(abi::StencilFaceState {
                compare: value.compare.try_into().ok()?,
                fail_op: value.fail_op.try_into().ok()?,
                depth_fail_op: value.depth_fail_op.try_into().ok()?,
                pass_op: value.pass_op.try_into().ok()?,
            })
        };
        convert().ok_or(value)
    }
}

impl From<abi::BindingType> for wgpu::BindingType {
    fn from(value: abi::BindingType) -> Self {
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
                min_binding_size: min_binding_size.and_then(wgpu::BufferSize::new),
            },
            abi::BindingType::Sampler(kind) => wgpu::BindingType::Sampler(kind.into()),
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
                view_dimension: view_dimension.into(),
                multisampled,
            },
            abi::BindingType::StorageTexture(abi::StorageTextureBindingLayout {
                access,
                format,
                view_dimension,
            }) => wgpu::BindingType::StorageTexture {
                access: access.into(),
                format: format.into(),
                view_dimension: view_dimension.into(),
            },
        }
    }
}

impl TryFrom<wgpu::BindingType> for abi::BindingType {
    type Error = wgpu::BindingType;

    fn try_from(value: wgpu::BindingType) -> Result<Self, Self::Error> {
        let convert = || {
            Some(match value {
                wgpu::BindingType::Buffer {
                    ty,
                    has_dynamic_offset,
                    min_binding_size,
                } => abi::BindingType::Buffer(abi::BufferBindingLayout {
                    kind: match ty {
                        wgpu::BufferBindingType::Uniform => abi::BufferBindingType::Uniform,
                        wgpu::BufferBindingType::Storage { read_only: false } => {
                            abi::BufferBindingType::Storage
                        }
                        wgpu::BufferBindingType::Storage { read_only: true } => {
                            abi::BufferBindingType::ReadOnlyStorage
                        }
                    },
                    has_dynamic_offset,
                    min_binding_size: min_binding_size.map(wgpu::BufferSize::get),
                }),
                wgpu::BindingType::Sampler(kind) => {
                    abi::BindingType::Sampler(kind.try_into().ok()?)
                }
                wgpu::BindingType::Texture {
                    sample_type,
                    view_dimension,
                    multisampled,
                } => abi::BindingType::Texture(abi::TextureBindingLayout {
                    sample_type: match sample_type {
                        wgpu::TextureSampleType::Float { filterable: true } => {
                            abi::TextureSampleType::FloatFilterable
                        }
                        wgpu::TextureSampleType::Float { filterable: false } => {
                            abi::TextureSampleType::FloatUnfilterable
                        }
                        wgpu::TextureSampleType::Depth => abi::TextureSampleType::Depth,
                        wgpu::TextureSampleType::Sint => abi::TextureSampleType::Sint,
                        wgpu::TextureSampleType::Uint => abi::TextureSampleType::Uint,
                    },
                    view_dimension: view_dimension.try_into().ok()?,
                    multisampled,
                }),
                wgpu::BindingType::StorageTexture {
                    access,
                    format,
                    view_dimension,
                } => abi::BindingType::StorageTexture(abi::StorageTextureBindingLayout {
                    access: access.try_into().ok()?,
                    format: format.try_into().ok()?,
                    view_dimension: view_dimension.try_into().ok()?,
                }),
                _ => return None,
            })
        };
        convert().ok_or(value)
    }
}

impl From<&abi::CompilationMessage> for wgpu::CompilationMessage {
    fn from(value: &abi::CompilationMessage) -> Self {
        wgpu::CompilationMessage {
            message: value.message.clone(),
            message_type: match value.message_type {
                abi::CompilationMessageType::Error => wgpu::CompilationMessageType::Error,
                abi::CompilationMessageType::Warning => wgpu::CompilationMessageType::Warning,
                abi::CompilationMessageType::Info => wgpu::CompilationMessageType::Info,
            },
            location: value
                .location
                .as_ref()
                .map(|location| wgpu::SourceLocation {
                    line_number: location.line_number,
                    line_position: location.line_position,
                    offset: location.offset,
                    length: location.length,
                }),
        }
    }
}

impl From<&wgpu::CompilationMessage> for abi::CompilationMessage {
    fn from(value: &wgpu::CompilationMessage) -> Self {
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
}
