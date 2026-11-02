use super::*;

#[test]
fn a_storage_usage_a_feature_allows_is_refused_until_it_is_requested() {
    let storage = texture(
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureUsages::STORAGE_BINDING,
    );
    assert!(allowed(wgpu::Features::empty()).texture(&storage).is_err());
    assert_eq!(
        allowed(wgpu::Features::BGRA8UNORM_STORAGE).texture(&storage),
        Ok(())
    );
    let unoffered = texture(
        wgpu::TextureFormat::Rg11b10Ufloat,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    assert_eq!(
        allowed(wgpu::Features::empty()).texture(&unoffered),
        Ok(()),
        "a usage no offered feature allows is left for the host to refuse"
    );
    let entries = [wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: wgpu::TextureFormat::Bgra8Unorm,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }];
    let layout = wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &entries,
    };
    assert!(
        allowed(wgpu::Features::empty())
            .group_layout(&layout)
            .is_err()
    );
    assert_eq!(
        allowed(wgpu::Features::BGRA8UNORM_STORAGE).group_layout(&layout),
        Ok(())
    );
}
