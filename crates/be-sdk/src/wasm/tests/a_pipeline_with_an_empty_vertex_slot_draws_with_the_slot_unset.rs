use super::*;

/// A pipeline whose one vertex buffer slot is empty, drawn with the slot unset in a pass
/// and in a bundle, as WebGPU's optional vertex buffer layouts allow.
#[test]
fn a_pipeline_with_an_empty_vertex_slot_draws_with_the_slot_unset() {
    let (device, queue) = gpu();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("fill"),
        source: wgpu::ShaderSource::Wgsl(FILL.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("fill with an empty slot"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: None,
            compilation_options: Default::default(),
            buffers: &[None],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: None,
            compilation_options: Default::default(),
            targets: &[Some(FILLED.into())],
        }),
        multiview_mask: None,
        cache: None,
    });
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut bundling = device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
        label: Some("fill"),
        color_formats: &[Some(FILLED)],
        depth_stencil: None,
        sample_count: 1,
        multiview: None,
    });
    bundling.set_pipeline(&pipeline);
    bundling.set_vertex_buffer(0, None::<wgpu::BufferSlice<'_>>);
    bundling.draw(0..3, 0..1);
    let bundle = bundling.finish(&wgpu::RenderBundleDescriptor {
        label: Some("fill"),
    });
    for bundled in [false, true] {
        let target = filled_target(&device);
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[cleared(&view)],
                ..Default::default()
            });
            match bundled {
                true => pass.execute_bundles([&bundle]),
                false => {
                    pass.set_pipeline(&pipeline);
                    pass.set_vertex_buffer(0, None::<wgpu::BufferSlice<'_>>);
                    pass.draw(0..3, 0..1);
                }
            }
        }
        assert_eq!(
            texels(&device, &queue, encoder, &target),
            vec![MAGENTA; 16],
            "the draw with the slot unset filled the target (bundled: {bundled})"
        );
    }
    let popped = pin!(scope.pop()).poll(&mut Context::from_waker(Waker::noop()));
    assert!(
        matches!(popped, Poll::Ready(None)),
        "unsetting an empty slot was refused: {popped:?}"
    );
}
