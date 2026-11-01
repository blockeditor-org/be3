use block_gpu_abi::device as abi;

use crate::objects::{
    BindGroupLayout, Buffer, PipelineLayout, Sampler, ShaderModule, Texture, TextureView,
};

fn unsupported(name: &str, value: &dyn core::fmt::Debug) -> ! {
    panic!("the plugin gpu abi does not support {name} {value:?}")
}

pub(crate) fn to_abi<A: TryFrom<W>, W: Copy + core::fmt::Debug>(value: W) -> A {
    A::try_from(value).unwrap_or_else(|_| unsupported(core::any::type_name::<W>(), &value))
}

pub(crate) fn constants(options: &wgpu::PipelineCompilationOptions<'_>) -> Vec<(String, f64)> {
    options
        .constants
        .iter()
        .map(|&(key, value)| (key.to_owned(), value))
        .collect()
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
        aspect: to_abi(value.aspect),
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

pub(crate) fn size(value: Option<wgpu::BufferSize>) -> Option<u64> {
    value.map(wgpu::BufferSize::get)
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
