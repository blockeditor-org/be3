use super::*;

#[test]
fn debug_groups_reach_the_host_and_must_balance() {
    let (device, _queue) = gpu();
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.push_debug_group("frame");
    encoder.insert_debug_marker("begin");
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.push_debug_group("compute");
        pass.insert_debug_marker("dispatch");
        pass.pop_debug_group();
    }
    {
        let target = filled_target(&device);
        let view = target.create_view(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[cleared(&view)],
            ..Default::default()
        });
        pass.push_debug_group("render");
        pass.insert_debug_marker("draw");
        pass.pop_debug_group();
    }
    encoder.pop_debug_group();
    let _balanced = encoder.finish();
    let popped = pin!(scope.pop()).poll(&mut Context::from_waker(Waker::noop()));
    assert!(
        matches!(popped, Poll::Ready(None)),
        "balanced groups are fine: {popped:?}"
    );

    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.pop_debug_group();
    }
    let _unbalanced = encoder.finish();
    let refused = refusal(scope);
    assert!(refused.to_lowercase().contains("debug"), "{refused}");
}
