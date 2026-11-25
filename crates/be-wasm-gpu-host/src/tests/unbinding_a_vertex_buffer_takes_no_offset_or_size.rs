use super::*;

/// `set-vertex-buffer` with no buffer unbinds its slot, which wgpu-core and WebGPU allow only
/// with an offset of 0 and no size.
#[test]
fn unbinding_a_vertex_buffer_takes_no_offset_or_size() {
    let mut gpu = gpu();
    let encoder = gpu
        .create_render_bundle_encoder(abi::RenderBundleEncoderDescriptor {
            label: None,
            color_formats: vec![Some(abi::TextureFormat::Rgba8Unorm)],
            depth_stencil: None,
            sample_count: 1,
        })
        .unwrap();
    for size in [None, Some(0)] {
        gpu.render_bundle_encoder_set_vertex_buffer(encoder, 0, None, 0, size);
        assert_eq!(gpu.take_error(), None, "unbinding with a size of {size:?}");
    }
    gpu.render_bundle_encoder_set_vertex_buffer(encoder, 0, None, 8, None);
    let error = gpu.take_error().expect("the offset is reported");
    assert!(error.contains("offset of 0, not 8"), "{error}");
    gpu.render_bundle_encoder_set_vertex_buffer(encoder, 0, None, 0, Some(16));
    let error = gpu.take_error().expect("the size is reported");
    assert!(error.contains("no size, not 16"), "{error}");
}
