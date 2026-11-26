use super::web::*;
use super::*;

#[test]
fn unbinding_a_vertex_buffer_takes_no_offset_or_size() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let encoder = bundling(&mut gpu, device);
    let unbind = |gpu: &mut Gpu, offset, size| {
        scoped(gpu, device, |gpu| {
            gpu.gpu_render_bundle_encoder_set_vertex_buffer(encoder, 0, None, offset, size)
        })
    };
    for size in [None, Some(0)] {
        assert_eq!(unbind(&mut gpu, None, size), None, "unbinding with a size of {size:?}");
    }
    let error = unbind(&mut gpu, Some(8), None).expect("the offset is refused");
    assert!(error.contains("offset of 0, not 8"), "{error}");
    let error = unbind(&mut gpu, None, Some(16)).expect("the size is refused");
    assert!(error.contains("no size, not 16"), "{error}");
}
