use super::*;

#[test]
fn a_render_bundle_replays_its_draws_in_a_pass() {
    let (device, queue) = gpu();
    let pipeline = filling(&device);
    let mut bundling = device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
        label: Some("fill"),
        color_formats: &[Some(FILLED)],
        depth_stencil: None,
        sample_count: 1,
        multiview: None,
    });
    bundling.set_pipeline(&pipeline);
    bundling.draw(0..3, 0..1);
    let bundle = bundling.finish(&wgpu::RenderBundleDescriptor { label: Some("fill") });
    let target = filled_target(&device);
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[cleared(&view)],
            ..Default::default()
        });
        pass.execute_bundles([&bundle]);
    }
    assert_eq!(
        texels(&device, &queue, encoder, &target),
        vec![MAGENTA; 16],
        "the bundle's draw filled the target"
    );
}
