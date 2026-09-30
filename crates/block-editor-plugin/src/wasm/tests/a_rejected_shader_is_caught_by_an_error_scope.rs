use super::*;

#[test]
fn a_rejected_shader_is_caught_by_an_error_scope() {
    let (device, _queue) = block_gpu_guest::device_and_queue();
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let _module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl("this is not a shader".into()),
    });
    let popped = pin!(scope.pop()).poll(&mut Context::from_waker(Waker::noop()));
    let Poll::Ready(Some(error)) = popped else {
        panic!("the rejected shader was not caught by the error scope");
    };
    assert!(
        matches!(error, wgpu::Error::Validation { .. }),
        "the rejected shader was caught as {error:?}"
    );
}
