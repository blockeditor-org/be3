use be_wasm_gpu_abi::guest::webgpu as gpu;
use be_wasm_gpu_abi::webgpu::webgpu as web;
use be_wasm_gpu_convert::TryConvert;

use crate::objects::{
    BindGroupLayout, Buffer, PipelineLayout, QuerySet, Sampler, ShaderModule, Texture, TextureView,
};

fn unsupported(name: &str, value: &dyn core::fmt::Debug) -> ! {
    panic!("wasi:webgpu has no {name} {value:?}")
}

/// A wgpu value as wasi:webgpu's: one WebGPU lacks is a bug in the app, as it would be in a
/// browser.
pub(crate) fn to_web<A, W: TryConvert<A> + Copy + core::fmt::Debug>(value: W) -> A {
    value
        .try_convert()
        .unwrap_or_else(|_| unsupported(core::any::type_name::<W>(), &value))
}

pub(crate) fn label(value: Option<&str>) -> Option<String> {
    value.map(str::to_owned)
}

/// A pipeline stage's constants, as the record resource wasi:webgpu takes, which must live until
/// the pipeline is made.
pub(crate) fn constants(
    options: &wgpu::PipelineCompilationOptions<'_>,
) -> Option<gpu::RecordGpuPipelineConstantValue> {
    if options.constants.is_empty() {
        return None;
    }
    let record = gpu::RecordGpuPipelineConstantValue::new();
    for &(key, value) in options.constants {
        record.add(key, value);
    }
    Some(record)
}

pub(crate) fn extent(value: wgpu::Extent3d) -> web::GpuExtent3D {
    web::GpuExtent3D {
        width: value.width,
        height: Some(value.height),
        depth_or_array_layers: Some(value.depth_or_array_layers),
    }
}

pub(crate) fn color(value: wgpu::Color) -> web::GpuColor {
    web::GpuColor {
        r: value.r,
        g: value.g,
        b: value.b,
        a: value.a,
    }
}

pub(crate) fn texel_copy_texture(
    value: wgpu::TexelCopyTextureInfo<'_>,
) -> web::GpuTexelCopyTextureInfo {
    web::GpuTexelCopyTextureInfo {
        texture: texture_handle(value.texture),
        mip_level: Some(value.mip_level),
        origin: Some(web::GpuOrigin3D {
            x: Some(value.origin.x),
            y: Some(value.origin.y),
            z: Some(value.origin.z),
        }),
        aspect: Some(to_web(value.aspect)),
    }
}

pub(crate) fn texel_copy_buffer(
    value: wgpu::TexelCopyBufferInfo<'_>,
) -> web::GpuTexelCopyBufferInfo {
    web::GpuTexelCopyBufferInfo {
        buffer: custom::<Buffer>(value.buffer.as_custom()).0.handle(),
        offset: Some(value.layout.offset),
        bytes_per_row: value.layout.bytes_per_row,
        rows_per_image: value.layout.rows_per_image,
    }
}

pub(crate) fn texel_copy_buffer_layout(
    value: wgpu::TexelCopyBufferLayout,
) -> web::GpuTexelCopyBufferLayout {
    web::GpuTexelCopyBufferLayout {
        offset: Some(value.offset),
        bytes_per_row: value.bytes_per_row,
        rows_per_image: value.rows_per_image,
    }
}

pub(crate) fn size(value: Option<wgpu::BufferSize>) -> Option<u64> {
    value.map(wgpu::BufferSize::get)
}

pub(crate) fn binding_resource(value: &wgpu::BindingResource<'_>) -> web::GpuBindingResource {
    match value {
        wgpu::BindingResource::Buffer(binding) => {
            web::GpuBindingResource::GpuBufferBinding(web::GpuBufferBinding {
                buffer: custom::<Buffer>(binding.buffer.as_custom()).0.handle(),
                offset: Some(binding.offset),
                size: size(binding.size),
            })
        }
        wgpu::BindingResource::Sampler(sampler) => {
            web::GpuBindingResource::GpuSampler(custom::<Sampler>(sampler.as_custom()).0.handle())
        }
        wgpu::BindingResource::TextureView(view) => web::GpuBindingResource::GpuTextureView(
            custom::<TextureView>(view.as_custom()).0.handle(),
        ),
        other => unsupported("binding resource", &other),
    }
}

/// A bind group layout entry's binding, as WebGPU names one of its four kinds.
pub(crate) fn layout_entry(entry: &wgpu::BindGroupLayoutEntry) -> web::GpuBindGroupLayoutEntry {
    let mut web_entry = web::GpuBindGroupLayoutEntry {
        binding: entry.binding,
        visibility: web::GpuShaderStage::from_bits(entry.visibility.bits() & 0x7),
        buffer: None,
        sampler: None,
        texture: None,
        storage_texture: None,
    };
    if entry.count.is_some() {
        unsupported("binding array", &entry.count);
    }
    match entry.ty {
        wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset,
            min_binding_size,
        } => {
            web_entry.buffer = Some(web::GpuBufferBindingLayout {
                r#type: Some(match ty {
                    wgpu::BufferBindingType::Uniform => web::GpuBufferBindingType::Uniform,
                    wgpu::BufferBindingType::Storage { read_only: false } => {
                        web::GpuBufferBindingType::Storage
                    }
                    wgpu::BufferBindingType::Storage { read_only: true } => {
                        web::GpuBufferBindingType::ReadOnlyStorage
                    }
                }),
                has_dynamic_offset: Some(has_dynamic_offset),
                min_binding_size: min_binding_size.map(wgpu::BufferSize::get),
            });
        }
        wgpu::BindingType::Sampler(kind) => {
            web_entry.sampler = Some(web::GpuSamplerBindingLayout {
                r#type: Some(to_web(kind)),
            });
        }
        wgpu::BindingType::Texture {
            sample_type,
            view_dimension,
            multisampled,
        } => {
            web_entry.texture = Some(web::GpuTextureBindingLayout {
                sample_type: Some(match sample_type {
                    wgpu::TextureSampleType::Float { filterable: true } => {
                        web::GpuTextureSampleType::Float
                    }
                    wgpu::TextureSampleType::Float { filterable: false } => {
                        web::GpuTextureSampleType::UnfilterableFloat
                    }
                    wgpu::TextureSampleType::Depth => web::GpuTextureSampleType::Depth,
                    wgpu::TextureSampleType::Sint => web::GpuTextureSampleType::Sint,
                    wgpu::TextureSampleType::Uint => web::GpuTextureSampleType::Uint,
                }),
                view_dimension: Some(to_web(view_dimension)),
                multisampled: Some(multisampled),
            });
        }
        wgpu::BindingType::StorageTexture {
            access,
            format,
            view_dimension,
        } => {
            web_entry.storage_texture = Some(web::GpuStorageTextureBindingLayout {
                access: Some(to_web(access)),
                format: to_web(format),
                view_dimension: Some(to_web(view_dimension)),
            });
        }
        other => unsupported("binding type", &other),
    }
    web_entry
}

pub(crate) fn custom<T>(value: Option<&T>) -> &T {
    match value {
        Some(value) => value,
        None => panic!("a resource from another wgpu backend reached wasi:webgpu"),
    }
}

pub(crate) fn texture_handle(value: &wgpu::Texture) -> web::GpuTexture {
    custom::<Texture>(value.as_custom()).0.handle()
}

pub(crate) fn view_handle(value: &wgpu::TextureView) -> web::GpuTextureView {
    custom::<TextureView>(value.as_custom()).0.handle()
}

pub(crate) fn layout_handle(value: &wgpu::PipelineLayout) -> web::GpuPipelineLayout {
    custom::<PipelineLayout>(value.as_custom()).0.handle()
}

pub(crate) fn group_layout_handle(value: &wgpu::BindGroupLayout) -> web::GpuBindGroupLayout {
    custom::<BindGroupLayout>(value.as_custom()).0.handle()
}

pub(crate) fn module_handle(value: &wgpu::ShaderModule) -> web::GpuShaderModule {
    custom::<ShaderModule>(value.as_custom()).0.handle()
}

pub(crate) fn query_set_handle(value: &wgpu::QuerySet) -> web::GpuQuerySet {
    custom::<QuerySet>(value.as_custom()).0.handle()
}

pub(crate) fn layout(value: Option<&wgpu::PipelineLayout>) -> web::GpuLayoutMode {
    match value {
        Some(layout) => web::GpuLayoutMode::Specific(layout_handle(layout)),
        None => web::GpuLayoutMode::Auto,
    }
}
