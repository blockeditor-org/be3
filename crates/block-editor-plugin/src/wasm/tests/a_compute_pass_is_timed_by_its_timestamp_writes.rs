use super::*;

#[test]
fn a_compute_pass_is_timed_by_its_timestamp_writes() {
    let (device, queue) = block_gpu_guest::request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TIMESTAMP_QUERY,
        required_limits: block_gpu_guest::limits(),
        ..Default::default()
    })
    .unwrap();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl("@compute @workgroup_size(1) fn idle() {}".into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: None,
        compilation_options: Default::default(),
        cache: None,
    });
    let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("timestamps"),
        ty: wgpu::QueryType::Timestamp,
        count: 2,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set: &queries,
                beginning_of_pass_write_index: Some(0),
                end_of_pass_write_index: Some(1),
            }),
        });
        pass.set_pipeline(&pipeline);
        pass.dispatch_workgroups(1, 1, 1);
    }
    let stamps = resolved(&device, &queue, encoder, &queries, 2);
    assert!(stamps[1] > 0, "the pass was timed: {stamps:?}");
    assert!(
        stamps[0] <= stamps[1],
        "the pass ended after it began: {stamps:?}"
    );
}
