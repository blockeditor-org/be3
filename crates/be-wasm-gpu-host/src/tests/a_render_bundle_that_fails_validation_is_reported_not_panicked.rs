use super::web::*;
use super::*;

#[test]
fn a_render_bundle_that_fails_validation_is_reported_not_panicked() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let encoder = bundling(&mut gpu, device);
    let error = scoped(&mut gpu, device, |gpu| {
        gpu.gpu_render_bundle_encoder_draw(encoder, 3, None, None, None);
        gpu.gpu_render_bundle_encoder_finish(encoder, None);
    })
    .expect("a draw without a pipeline is refused");
    assert!(error.contains("pipeline"), "{error}");
    for (formats, sample_count) in [(9, 1), (1, 3)] {
        let refused = scoped(&mut gpu, device, |gpu| {
            gpu.gpu_device_create_render_bundle_encoder(
                device,
                GpuRenderBundleEncoderDescriptor {
                    color_formats: vec![Some(GpuTextureFormat::Rgba8unorm); formats],
                    depth_stencil_format: None,
                    sample_count: Some(sample_count),
                    depth_read_only: None,
                    stencil_read_only: None,
                    label: None,
                },
            );
        });
        assert!(
            refused.is_some(),
            "{formats} formats at {sample_count} samples"
        );
    }
}
