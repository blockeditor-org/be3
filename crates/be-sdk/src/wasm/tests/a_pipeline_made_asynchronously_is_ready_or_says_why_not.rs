use super::*;

#[test]
fn a_pipeline_made_asynchronously_is_ready_or_says_why_not() {
    let (device, _queue) = be_wasm_gpu_guest::request_device(&wgpu::DeviceDescriptor {
        required_limits: be_wasm_gpu_guest::limits(),
        ..Default::default()
    })
    .unwrap();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl("@compute @workgroup_size(1) fn main() {}".into()),
    });
    let made = |entry_point| {
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let made = pin!(be_wasm_gpu_guest::create_compute_pipeline_async(
            &device,
            &wgpu::ComputePipelineDescriptor {
                label: None,
                layout: None,
                module: &module,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            },
        ))
        .poll(&mut Context::from_waker(Waker::noop()));
        let Poll::Ready(made) = made else {
            panic!("the host did not settle the pipeline at once");
        };
        let popped = pin!(scope.pop()).poll(&mut Context::from_waker(Waker::noop()));
        assert!(
            matches!(popped, Poll::Ready(None)),
            "making the pipeline raised {popped:?}"
        );
        made.map(|_| ())
    };
    assert_eq!(made("main"), Ok(()));
    let refused = made("missing").expect_err("a pipeline with no such entry point was made");
    assert!(refused.contains("missing"), "it was refused as {refused}");
}
