use super::*;

fn view(gpu: &mut Gpu, texture: abi::Texture) -> Option<abi::TextureView> {
    gpu.texture_create_view(
        texture,
        abi::TextureViewDescriptor {
            label: None,
            format: None,
            dimension: None,
            usage: None,
            aspect: abi::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        },
    )
}

#[test]
fn only_a_texture_the_plugin_created_is_destroyed() {
    let mut gpu = gpu();
    let surface = gpu.surface_new(0);
    gpu.surface_configure(surface, configuration(4, 4));
    let acquired = gpu
        .surface_acquire(surface)
        .expect("the configured surface should be acquired");
    gpu.texture_destroy(acquired);
    assert!(
        view(&mut gpu, acquired).is_some(),
        "the surface's texture was destroyed by the plugin: {:?}",
        gpu.take_error()
    );
    let made = gpu
        .create_texture(abi::TextureDescriptor {
            label: None,
            size: abi::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: abi::TextureDimension::D2,
            format: abi::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING.bits(),
            view_formats: Vec::new(),
        })
        .expect("the texture should be made");
    assert!(view(&mut gpu, made).is_some());
    assert_eq!(gpu.take_error(), None);
    gpu.texture_destroy(made);
    assert_eq!(gpu.take_error(), None);
    assert!(
        view(&mut gpu, made).is_none(),
        "a destroyed texture made a view"
    );
    assert!(gpu.take_error().is_some());
}
