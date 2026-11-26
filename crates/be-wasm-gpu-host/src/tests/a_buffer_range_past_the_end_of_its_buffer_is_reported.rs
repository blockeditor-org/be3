use super::web::*;
use super::*;

#[test]
fn a_buffer_range_past_the_end_of_its_buffer_is_reported() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let buffer = gpu.gpu_device_create_buffer(device, web::buffer(GpuBufferUsage::VERTEX));
    let encoder = bundling(&mut gpu, device);
    let set = |gpu: &mut Gpu, offset, size| {
        scoped(gpu, device, |gpu| {
            gpu.gpu_render_bundle_encoder_set_vertex_buffer(encoder, 0, Some(buffer), Some(offset), size)
        })
    };
    assert_eq!(set(&mut gpu, 0, Some(256)), None);
    for (offset, size) in [(512, None), (128, Some(256)), (0, Some(0)), (8, Some(u64::MAX))] {
        let error = set(&mut gpu, offset, size).expect("the range is refused");
        assert!(error.contains("runs past the end"), "{error}");
    }
}
