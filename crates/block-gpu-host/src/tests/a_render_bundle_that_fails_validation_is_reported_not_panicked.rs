use super::*;

fn bundling(formats: usize, sample_count: u32) -> abi::RenderBundleEncoderDescriptor {
    abi::RenderBundleEncoderDescriptor {
        label: Some("bundle".into()),
        color_formats: vec![Some(abi::TextureFormat::Rgba8Unorm); formats],
        depth_stencil: None,
        sample_count,
    }
}

#[test]
fn a_render_bundle_that_fails_validation_is_reported_not_panicked() {
    let mut gpu = gpu();
    let encoder = gpu.create_render_bundle_encoder(bundling(1, 1)).unwrap();
    gpu.render_bundle_encoder_draw(encoder, 3, 1, 0, 0);
    let finished = gpu.render_bundle_encoder_finish(
        encoder,
        abi::RenderBundleDescriptor { label: None },
    );
    assert_eq!(finished, None);
    let error = gpu.take_error().expect("a draw without a pipeline is reported");
    assert!(error.contains("pipeline"), "{error}");
    for (formats, sample_count) in [(9, 1), (1, 3)] {
        let refused = gpu.create_render_bundle_encoder(bundling(formats, sample_count));
        assert_eq!(refused, None);
        assert!(gpu.take_error().is_some(), "{formats} formats at {sample_count} samples");
    }
}
