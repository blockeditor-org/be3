use super::*;

#[test]
fn timestamps_resolve_to_nanoseconds_coarsened_to_a_tenth_of_a_millisecond() {
    assert!(
        block_gpu_guest::features().contains(wgpu::Features::TIMESTAMP_QUERY),
        "the test runner's adapter offers timestamps"
    );
    let (device, queue) = block_gpu_guest::request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TIMESTAMP_QUERY,
        required_limits: block_gpu_guest::limits(),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(queue.get_timestamp_period(), 1.0, "a tick is a nanosecond");
    let pipeline = filling(&device);
    let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("timestamps"),
        ty: wgpu::QueryType::Timestamp,
        count: 2,
    });
    let target = filled_target(&device);
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[cleared(&view)],
            timestamp_writes: Some(wgpu::RenderPassTimestampWrites {
                query_set: &queries,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            }),
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
    }
    let stamps = resolved(&device, &queue, encoder, &queries, 2);
    assert!(stamps[1] > 0, "the pass was timed: {stamps:?}");
    assert!(stamps[0] <= stamps[1], "the pass ended after it began: {stamps:?}");
    for stamp in stamps {
        assert_eq!(stamp % 100_000, 0, "{stamp} is coarsened to 100 microseconds");
    }
}
