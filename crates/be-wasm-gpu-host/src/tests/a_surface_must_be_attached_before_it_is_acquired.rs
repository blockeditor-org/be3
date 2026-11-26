use super::*;

/// A surface the host draws itself (a root's, `attach_surface`) has the texture the host gave
/// it, and none until then.
#[test]
fn a_surface_must_be_attached_before_it_is_acquired() {
    let mut gpu = gpu();
    assert!(gpu.surface(0).is_none());
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
    gpu.attach_surface(0, texture.clone());
    assert_eq!(
        gpu.surface(0).map(|(shown, _)| shown.clone()),
        Some(texture)
    );
}
