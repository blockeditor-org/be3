use super::*;

#[test]
fn a_surface_must_be_attached_before_it_is_acquired() {
    let mut gpu = gpu();
    let surface = gpu.surface_new(0);
    assert_eq!(gpu.surface_acquire(surface), None);
    assert!(gpu.take_error().is_some());
    let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    gpu.attach_surface(0, texture);
    let acquired = gpu
        .surface_acquire(surface)
        .expect("the attached surface should be acquired");
    assert!(gpu.texture_describe(acquired).is_some());
    assert_eq!(gpu.take_error(), None);
}
