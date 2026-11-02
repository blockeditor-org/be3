use super::*;

#[test]
fn an_occlusion_query_counts_only_the_samples_its_draws_pass() {
    let (device, queue) = gpu();
    let pipeline = filling(&device);
    let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("occlusion"),
        ty: wgpu::QueryType::Occlusion,
        count: 2,
    });
    let target = filled_target(&device);
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[cleared(&view)],
            occlusion_query_set: Some(&queries),
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.begin_occlusion_query(0);
        pass.draw(0..3, 0..1);
        pass.end_occlusion_query();
        pass.begin_occlusion_query(1);
        pass.draw(0..0, 0..1);
        pass.end_occlusion_query();
    }
    let counts = resolved(&device, &queue, encoder, &queries, 2);
    assert!(
        counts[0] > 0,
        "the covering draw passed samples: {counts:?}"
    );
    assert_eq!(counts[1], 0, "the empty draw passed none");
}
