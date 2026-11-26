use super::*;

/// Polls `made` until it is ready, waiting on the gpu between polls: the host makes a pipeline
/// off the app's thread, so it may not be ready at the first.
fn ready<T>(made: impl Future<Output = T>) -> T {
    let mut made = pin!(made);
    loop {
        if let Poll::Ready(made) = made.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
            return made;
        }
        be_wasm_gpu_guest::wait();
    }
}

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
        let made = ready(be_wasm_gpu_guest::create_compute_pipeline_async(
            &device,
            &wgpu::ComputePipelineDescriptor {
                label: None,
                layout: None,
                module: &module,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            },
        ));
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

    let drawing = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(
            "@vertex fn vertex() -> @builtin(position) vec4f { return vec4f(0.0); }
             @fragment fn fragment() -> @location(0) vec4f { return vec4f(1.0); }"
                .into(),
        ),
    });
    let rendered = ready(be_wasm_gpu_guest::create_render_pipeline_async(
        &device,
        &wgpu::RenderPipelineDescriptor {
            label: None,
            layout: None,
            vertex: wgpu::VertexState {
                module: &drawing,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &drawing,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
            }),
            multiview_mask: None,
            cache: None,
        },
    ));
    assert!(
        rendered.is_ok(),
        "a render pipeline is made asynchronously too"
    );
}
