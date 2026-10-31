use super::*;

#[test]
fn a_texture_whose_format_needs_an_unrequested_feature_is_refused() {
    let (device, _queue) = block_gpu_guest::request_device(&wgpu::DeviceDescriptor {
        required_limits: block_gpu_guest::limits(),
        ..Default::default()
    })
    .unwrap();
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let _texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bc1RgbaUnorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let popped = pin!(scope.pop()).poll(&mut Context::from_waker(Waker::noop()));
    let Poll::Ready(Some(error)) = popped else {
        panic!("the texture was not refused");
    };
    assert!(
        error.to_string().contains("TEXTURE_COMPRESSION_BC"),
        "the texture was refused as {error}"
    );
}
