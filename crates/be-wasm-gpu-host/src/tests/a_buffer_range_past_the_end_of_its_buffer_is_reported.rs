use super::*;

#[test]
fn a_buffer_range_past_the_end_of_its_buffer_is_reported() {
    let mut gpu = gpu();
    let buffer = gpu.create_buffer(buffer_descriptor()).unwrap();
    let encoder = gpu
        .create_render_bundle_encoder(abi::RenderBundleEncoderDescriptor {
            label: None,
            color_formats: vec![Some(abi::TextureFormat::Rgba8Unorm)],
            depth_stencil: None,
            sample_count: 1,
        })
        .unwrap();
    gpu.render_bundle_encoder_set_vertex_buffer(encoder, 0, buffer, 0, Some(256));
    assert_eq!(gpu.take_error(), None);
    for (offset, size) in [
        (512, None),
        (128, Some(256)),
        (0, Some(0)),
        (8, Some(u64::MAX)),
    ] {
        gpu.render_bundle_encoder_set_vertex_buffer(encoder, 0, buffer, offset, size);
        let error = gpu.take_error().expect("the range is reported");
        assert!(error.contains("runs past the end"), "{error}");
    }
}
