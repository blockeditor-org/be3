use crate::features::{Allowed, enabled, requested};

fn allowed(requested: wgpu::Features) -> Allowed {
    Allowed {
        requested,
        offered: wgpu::Features::BGRA8UNORM_STORAGE
            | wgpu::Features::TEXTURE_COMPRESSION_BC
            | wgpu::Features::SHADER_F16
            | wgpu::Features::FLOAT32_BLENDABLE,
    }
}

fn texture(
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    }
}

mod a_device_is_refused_a_feature_or_limit_the_host_does_not_offer;
mod a_format_that_needs_a_feature_is_refused_until_it_is_requested;
mod a_shader_enabling_an_extension_needs_its_feature;
mod a_storage_usage_a_feature_allows_is_refused_until_it_is_requested;
mod blending_on_a_float32_target_needs_its_feature;
mod the_extensions_a_shader_enables_are_read_past_comments;
