use super::*;

#[test]
fn a_render_bundle_that_draws_without_a_pipeline_is_refused() {
    let (device, _queue) = gpu();
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut bundling = device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
        label: None,
        color_formats: &[Some(FILLED)],
        depth_stencil: None,
        sample_count: 1,
        multiview: None,
    });
    bundling.draw(0..3, 0..1);
    let _bundle = bundling.finish(&wgpu::RenderBundleDescriptor::default());
    let refused = refusal(scope);
    assert!(
        refused.contains("pipeline"),
        "the bundle was refused as {refused}"
    );
}
