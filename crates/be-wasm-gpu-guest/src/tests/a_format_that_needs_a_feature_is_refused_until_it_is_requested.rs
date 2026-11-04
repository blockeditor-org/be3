use super::*;

#[test]
fn a_format_that_needs_a_feature_is_refused_until_it_is_requested() {
    let compressed = texture(
        wgpu::TextureFormat::Bc1RgbaUnorm,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    assert!(
        allowed(wgpu::Features::empty())
            .texture(&compressed)
            .is_err()
    );
    assert_eq!(
        allowed(wgpu::Features::TEXTURE_COMPRESSION_BC).texture(&compressed),
        Ok(())
    );
    let plain = texture(
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    assert_eq!(allowed(wgpu::Features::empty()).texture(&plain), Ok(()));
}
