use block_gpu_abi::device as abi;

use crate::objects::{
    BindGroupLayout, Buffer, PipelineLayout, Sampler, ShaderModule, Texture, TextureView,
};

fn unsupported(name: &str, value: &dyn core::fmt::Debug) -> ! {
    panic!("the plugin gpu abi does not support {name} {value:?}")
}

macro_rules! plain_map {
    ($function:ident, $reverse:ident, $from:path, $to:path, { $($variant:ident),+ $(,)? } $(, { $($named:ident = $value:expr),+ $(,)? })?) => {
        #[allow(unreachable_patterns)]
        pub(crate) fn $function(value: $from) -> $to {
            use $from as From;
            use $to as To;
            match value {
                $(From::$variant => To::$variant,)+
                other => {
                    $($(if other == $value {
                        return To::$named;
                    })+)?
                    unsupported(stringify!($from), &other)
                }
            }
        }

        #[allow(dead_code)]
        pub(crate) fn $reverse(value: $to) -> $from {
            use $from as From;
            use $to as To;
            match value {
                $(To::$variant => From::$variant,)+
                $($(To::$named => $value,)+)?
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

plain_map!(texture_format, wgpu_texture_format, wgpu::TextureFormat, abi::TextureFormat, {
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

plain_map!(texture_dimension, wgpu_texture_dimension, wgpu::TextureDimension, abi::TextureDimension, {
    D1, D2, D3,
});

plain_map!(texture_view_dimension, wgpu_texture_view_dimension, wgpu::TextureViewDimension, abi::TextureViewDimension, {
    D1, D2, D2Array, Cube, CubeArray, D3,
});

plain_map!(texture_aspect, wgpu_texture_aspect, wgpu::TextureAspect, abi::TextureAspect, {
    All, StencilOnly, DepthOnly,
});

plain_map!(address_mode, wgpu_address_mode, wgpu::AddressMode, abi::AddressMode, {
    ClampToEdge, Repeat, MirrorRepeat, ClampToBorder,
});

plain_map!(filter_mode, wgpu_filter_mode, wgpu::FilterMode, abi::FilterMode, {
    Nearest, Linear,
});

plain_map!(compare_function, wgpu_compare_function, wgpu::CompareFunction, abi::CompareFunction, {
    Never, Less, Equal, LessEqual, Greater, NotEqual, GreaterEqual, Always,
});

plain_map!(border_color, wgpu_border_color, wgpu::SamplerBorderColor, abi::SamplerBorderColor, {
    TransparentBlack, OpaqueBlack, OpaqueWhite, Zero,
});

plain_map!(vertex_format, wgpu_vertex_format, wgpu::VertexFormat, abi::VertexFormat, {
    Uint8x2, Uint8x4, Sint8x2, Sint8x4,
    Unorm8x2, Unorm8x4, Snorm8x2, Snorm8x4,
    Uint16x2, Uint16x4, Sint16x2, Sint16x4,
    Unorm16x2, Unorm16x4, Snorm16x2, Snorm16x4,
    Float16x2, Float16x4,
    Float32, Float32x2, Float32x3, Float32x4,
    Uint32, Uint32x2, Uint32x3, Uint32x4,
    Sint32, Sint32x2, Sint32x3, Sint32x4,
});

plain_map!(step_mode, wgpu_step_mode, wgpu::VertexStepMode, abi::VertexStepMode, {
    Vertex, Instance,
});

plain_map!(topology, wgpu_topology, wgpu::PrimitiveTopology, abi::PrimitiveTopology, {
    PointList, LineList, LineStrip, TriangleList, TriangleStrip,
});

plain_map!(index_format, wgpu_index_format, wgpu::IndexFormat, abi::IndexFormat, {
    Uint16, Uint32,
});

plain_map!(front_face, wgpu_front_face, wgpu::FrontFace, abi::FrontFace, { Ccw, Cw });

plain_map!(face, wgpu_face, wgpu::Face, abi::Face, { Front, Back });

plain_map!(polygon_mode, wgpu_polygon_mode, wgpu::PolygonMode, abi::PolygonMode, {
    Fill, Line, Point,
});

plain_map!(blend_factor, wgpu_blend_factor, wgpu::BlendFactor, abi::BlendFactor, {
    Zero, One, Src, OneMinusSrc, SrcAlpha, OneMinusSrcAlpha,
    Dst, OneMinusDst, DstAlpha, OneMinusDstAlpha,
    SrcAlphaSaturated, Constant, OneMinusConstant,
    Src1, OneMinusSrc1, Src1Alpha, OneMinusSrc1Alpha,
});

plain_map!(blend_operation, wgpu_blend_operation, wgpu::BlendOperation, abi::BlendOperation, {
    Add, Subtract, ReverseSubtract, Min, Max,
});

plain_map!(stencil_operation, wgpu_stencil_operation, wgpu::StencilOperation, abi::StencilOperation, {
    Keep, Zero, Replace, Invert,
    IncrementClamp, DecrementClamp, IncrementWrap, DecrementWrap,
});

plain_map!(sampler_binding, wgpu_sampler_binding, wgpu::SamplerBindingType, abi::SamplerBindingType, {
    Filtering, NonFiltering, Comparison,
});

plain_map!(storage_access, wgpu_storage_access, wgpu::StorageTextureAccess, abi::StorageTextureAccess, {
    WriteOnly, ReadOnly, ReadWrite,
});

plain_map!(store_op, wgpu_store_op, wgpu::StoreOp, abi::StoreOp, { Store, Discard });

#[allow(unreachable_patterns)]
pub(crate) fn mipmap_filter_mode(value: wgpu::MipmapFilterMode) -> abi::FilterMode {
    match value {
        wgpu::MipmapFilterMode::Nearest => abi::FilterMode::Nearest,
        wgpu::MipmapFilterMode::Linear => abi::FilterMode::Linear,
        other => unsupported("mipmap filter", &other),
    }
}

pub(crate) fn label(value: Option<&str>) -> Option<String> {
    value.map(str::to_owned)
}

pub(crate) fn texel_copy_texture(
    value: wgpu::TexelCopyTextureInfo<'_>,
) -> abi::TexelCopyTextureInfo {
    abi::TexelCopyTextureInfo {
        texture: texture_handle(value.texture),
        mip_level: value.mip_level,
        origin: abi::Origin3d {
            x: value.origin.x,
            y: value.origin.y,
            z: value.origin.z,
        },
        aspect: texture_aspect(value.aspect),
    }
}

pub(crate) fn texel_copy_buffer(value: wgpu::TexelCopyBufferInfo<'_>) -> abi::TexelCopyBufferInfo {
    abi::TexelCopyBufferInfo {
        buffer: custom::<Buffer>(value.buffer.as_custom()).0.handle(),
        layout: texel_copy_buffer_layout(value.layout),
    }
}

pub(crate) fn texel_copy_buffer_layout(
    value: wgpu::TexelCopyBufferLayout,
) -> abi::TexelCopyBufferLayout {
    abi::TexelCopyBufferLayout {
        offset: value.offset,
        bytes_per_row: value.bytes_per_row,
        rows_per_image: value.rows_per_image,
    }
}

pub(crate) fn extent(value: wgpu::Extent3d) -> abi::Extent3d {
    abi::Extent3d {
        width: value.width,
        height: value.height,
        depth_or_array_layers: value.depth_or_array_layers,
    }
}

pub(crate) fn size(value: Option<wgpu::BufferSize>) -> Option<u64> {
    value.map(wgpu::BufferSize::get)
}

pub(crate) fn binding_type(value: wgpu::BindingType) -> abi::BindingType {
    match value {
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
            min_binding_size: size(min_binding_size),
        }),
        wgpu::BindingType::Sampler(kind) => abi::BindingType::Sampler(sampler_binding(kind)),
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
            view_dimension: texture_view_dimension(view_dimension),
            multisampled,
        }),
        wgpu::BindingType::StorageTexture {
            access,
            format,
            view_dimension,
        } => abi::BindingType::StorageTexture(abi::StorageTextureBindingLayout {
            access: storage_access(access),
            format: texture_format(format),
            view_dimension: texture_view_dimension(view_dimension),
        }),
        other => unsupported("binding type", &other),
    }
}

pub(crate) fn binding_resource(value: &wgpu::BindingResource<'_>) -> abi::BindingResource {
    match value {
        wgpu::BindingResource::Buffer(binding) => {
            abi::BindingResource::Buffer(abi::BufferBinding {
                buffer: custom::<Buffer>(binding.buffer.as_custom()).0.handle(),
                offset: binding.offset,
                size: size(binding.size),
            })
        }
        wgpu::BindingResource::Sampler(sampler) => {
            abi::BindingResource::Sampler(custom::<Sampler>(sampler.as_custom()).0.handle())
        }
        wgpu::BindingResource::TextureView(view) => {
            abi::BindingResource::TextureView(custom::<TextureView>(view.as_custom()).0.handle())
        }
        other => unsupported("binding resource", &other),
    }
}

pub(crate) fn custom<T>(value: Option<&T>) -> &T {
    match value {
        Some(value) => value,
        None => panic!("a resource from another wgpu backend reached the plugin gpu abi"),
    }
}

pub(crate) fn texture_handle(value: &wgpu::Texture) -> abi::Texture {
    custom::<Texture>(value.as_custom()).0.handle()
}

pub(crate) fn view_handle(value: &wgpu::TextureView) -> abi::TextureView {
    custom::<TextureView>(value.as_custom()).0.handle()
}

pub(crate) fn layout_handle(value: &wgpu::PipelineLayout) -> abi::PipelineLayout {
    custom::<PipelineLayout>(value.as_custom()).0.handle()
}

pub(crate) fn group_layout_handle(value: &wgpu::BindGroupLayout) -> abi::BindGroupLayout {
    custom::<BindGroupLayout>(value.as_custom()).0.handle()
}

pub(crate) fn module_handle(value: &wgpu::ShaderModule) -> abi::ShaderModule {
    custom::<ShaderModule>(value.as_custom()).0.handle()
}

pub(crate) fn blend_component(value: wgpu::BlendComponent) -> abi::BlendComponent {
    abi::BlendComponent {
        src_factor: blend_factor(value.src_factor),
        dst_factor: blend_factor(value.dst_factor),
        operation: blend_operation(value.operation),
    }
}

pub(crate) fn stencil_face(value: wgpu::StencilFaceState) -> abi::StencilFaceState {
    abi::StencilFaceState {
        compare: compare_function(value.compare),
        fail_op: stencil_operation(value.fail_op),
        depth_fail_op: stencil_operation(value.depth_fail_op),
        pass_op: stencil_operation(value.pass_op),
    }
}

pub(crate) fn color(value: wgpu::Color) -> abi::Color {
    abi::Color {
        r: value.r,
        g: value.g,
        b: value.b,
        a: value.a,
    }
}
