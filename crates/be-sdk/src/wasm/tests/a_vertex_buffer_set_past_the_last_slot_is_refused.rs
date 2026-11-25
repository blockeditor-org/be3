use super::*;

/// A vertex buffer set at a slot past the device's last is an error the app hears, in a
/// pass and in a bundle, and the host does not panic on it.
#[test]
fn a_vertex_buffer_set_past_the_last_slot_is_refused() {
    let (device, queue) = gpu();
    let slot = device.limits().max_vertex_buffers;
    let vertices = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vertices"),
        size: 64,
        usage: wgpu::BufferUsages::VERTEX,
        mapped_at_creation: false,
    });

    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let target = filled_target(&device);
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[cleared(&view)],
            ..Default::default()
        });
        pass.set_vertex_buffer(slot, vertices.slice(..));
    }
    queue.submit([encoder.finish()]);
    let refused = refusal(scope);
    assert!(refused.contains("max_vertex_buffers"), "{refused}");

    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut bundling = device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
        label: Some("past the last slot"),
        color_formats: &[Some(FILLED)],
        depth_stencil: None,
        sample_count: 1,
        multiview: None,
    });
    bundling.set_vertex_buffer(slot, vertices.slice(..));
    let _bundle = bundling.finish(&wgpu::RenderBundleDescriptor { label: None });
    let refused = refusal(scope);
    assert!(refused.contains("max_vertex_buffers"), "{refused}");
}
