use super::*;

fn drawn_into(device: &wgpu::Device, view: &wgpu::TextureView) -> Option<wgpu::Error> {
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations::default(),
        })],
        ..Default::default()
    });
    drop(encoder.finish());
    let Poll::Ready(error) = pin!(scope.pop()).poll(&mut Context::from_waker(Waker::noop()))
    else {
        panic!("the error scope did not settle");
    };
    error
}

#[test]
fn a_texture_view_has_only_the_usages_it_asks_for() {
    let (device, _queue) = gpu();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let every = texture.create_view(&wgpu::TextureViewDescriptor::default());
    assert!(drawn_into(&device, &every).is_none());
    let sampled = texture.create_view(&wgpu::TextureViewDescriptor {
        usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
        ..Default::default()
    });
    assert!(
        drawn_into(&device, &sampled).is_some(),
        "a view without the render attachment usage was drawn into"
    );
}
