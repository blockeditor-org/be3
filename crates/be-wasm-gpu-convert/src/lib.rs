//! Conversions between the gpu ABI's types (be-wasm-gpu-abi) and wgpu's, which the guest's
//! backend (be-wasm-gpu-guest) and the host (be-wasm-gpu-host) share. A crate of its own, so the
//! ABI reaches a pure guest without wgpu, which on wasm32 links wasm-bindgen
//! (guides/app_libraries.md, Pure guests). They are traits of this crate's, since neither side's
//! types are.

use be_wasm_gpu_abi::device as abi;

/// `self` as the other side's `T`.
pub trait Convert<T> {
    fn convert(self) -> T;
}

/// `self` as the other side's `T`, or `self` back if `T` has nothing like it.
pub trait TryConvert<T>: Sized {
    fn try_convert(self) -> Result<T, Self>;
}

/// A conversion that always succeeds, as a `TryConvert` too.
macro_rules! infallible {
    ($($from:ty => $to:ty),* $(,)?) => {
        $(impl TryConvert<$to> for $from {
            fn try_convert(self) -> Result<$to, Self> {
                Ok(self.convert())
            }
        })*
    };
}

infallible!(wgpu::Extent3d => abi::Extent3d, wgpu::Color => abi::Color);

macro_rules! plain_map {
    ($abi:path, $wgpu:path, { $($variant:ident),+ $(,)? } $(, { $($named:ident = $value:expr),+ $(,)? })?) => {
        impl Convert<$wgpu> for $abi {
            fn convert(self) -> $wgpu {
                let value = self;
                use $abi as Abi;
                use $wgpu as Wgpu;
                match value {
                    $(Abi::$variant => Wgpu::$variant,)+
                    $($(Abi::$named => $value,)+)?
                }
            }
        }

        impl TryConvert<$abi> for $wgpu {
            #[allow(unreachable_patterns)]
            fn try_convert(self) -> Result<$abi, $wgpu> {
                let value = self;
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

impl Convert<wgpu::Extent3d> for abi::Extent3d {
    fn convert(self) -> wgpu::Extent3d {
        let value = self;
        wgpu::Extent3d {
            width: value.width,
            height: value.height,
            depth_or_array_layers: value.depth_or_array_layers,
        }
    }
}

impl Convert<abi::Extent3d> for wgpu::Extent3d {
    fn convert(self) -> abi::Extent3d {
        let value = self;
        abi::Extent3d {
            width: value.width,
            height: value.height,
            depth_or_array_layers: value.depth_or_array_layers,
        }
    }
}

impl Convert<wgpu::Color> for abi::Color {
    fn convert(self) -> wgpu::Color {
        let value = self;
        wgpu::Color {
            r: value.r,
            g: value.g,
            b: value.b,
            a: value.a,
        }
    }
}

impl Convert<abi::Color> for wgpu::Color {
    fn convert(self) -> abi::Color {
        let value = self;
        abi::Color {
            r: value.r,
            g: value.g,
            b: value.b,
            a: value.a,
        }
    }
}

impl Convert<wgpu::BlendComponent> for abi::BlendComponent {
    fn convert(self) -> wgpu::BlendComponent {
        let value = self;
        wgpu::BlendComponent {
            src_factor: value.src_factor.convert(),
            dst_factor: value.dst_factor.convert(),
            operation: value.operation.convert(),
        }
    }
}

impl TryConvert<abi::BlendComponent> for wgpu::BlendComponent {
    fn try_convert(self) -> Result<abi::BlendComponent, wgpu::BlendComponent> {
        let value = self;
        let convert = || {
            Some(abi::BlendComponent {
                src_factor: value.src_factor.try_convert().ok()?,
                dst_factor: value.dst_factor.try_convert().ok()?,
                operation: value.operation.try_convert().ok()?,
            })
        };
        convert().ok_or(value)
    }
}

impl Convert<wgpu::StencilFaceState> for abi::StencilFaceState {
    fn convert(self) -> wgpu::StencilFaceState {
        let value = self;
        wgpu::StencilFaceState {
            compare: value.compare.convert(),
            fail_op: value.fail_op.convert(),
            depth_fail_op: value.depth_fail_op.convert(),
            pass_op: value.pass_op.convert(),
        }
    }
}

impl TryConvert<abi::StencilFaceState> for wgpu::StencilFaceState {
    fn try_convert(self) -> Result<abi::StencilFaceState, wgpu::StencilFaceState> {
        let value = self;
        let convert = || {
            Some(abi::StencilFaceState {
                compare: value.compare.try_convert().ok()?,
                fail_op: value.fail_op.try_convert().ok()?,
                depth_fail_op: value.depth_fail_op.try_convert().ok()?,
                pass_op: value.pass_op.try_convert().ok()?,
            })
        };
        convert().ok_or(value)
    }
}

impl Convert<wgpu::BindingType> for abi::BindingType {
    fn convert(self) -> wgpu::BindingType {
        let value = self;
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
            abi::BindingType::Sampler(kind) => wgpu::BindingType::Sampler(kind.convert()),
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
                view_dimension: view_dimension.convert(),
                multisampled,
            },
            abi::BindingType::StorageTexture(abi::StorageTextureBindingLayout {
                access,
                format,
                view_dimension,
            }) => wgpu::BindingType::StorageTexture {
                access: access.convert(),
                format: format.convert(),
                view_dimension: view_dimension.convert(),
            },
        }
    }
}

impl TryConvert<abi::BindingType> for wgpu::BindingType {
    fn try_convert(self) -> Result<abi::BindingType, wgpu::BindingType> {
        let value = self;
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
                    abi::BindingType::Sampler(kind.try_convert().ok()?)
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
                    view_dimension: view_dimension.try_convert().ok()?,
                    multisampled,
                }),
                wgpu::BindingType::StorageTexture {
                    access,
                    format,
                    view_dimension,
                } => abi::BindingType::StorageTexture(abi::StorageTextureBindingLayout {
                    access: access.try_convert().ok()?,
                    format: format.try_convert().ok()?,
                    view_dimension: view_dimension.try_convert().ok()?,
                }),
                _ => return None,
            })
        };
        convert().ok_or(value)
    }
}

impl Convert<wgpu::CompilationMessage> for &abi::CompilationMessage {
    fn convert(self) -> wgpu::CompilationMessage {
        let value = self;
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

impl Convert<abi::CompilationMessage> for &wgpu::CompilationMessage {
    fn convert(self) -> abi::CompilationMessage {
        let value = self;
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
