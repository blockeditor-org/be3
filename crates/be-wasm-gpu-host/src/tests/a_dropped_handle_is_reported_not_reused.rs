use super::web::*;
use super::*;

#[test]
fn a_dropped_handle_is_reported_not_reused() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let queue = gpu.gpu_device_queue(device);
    let buffer = gpu.gpu_device_create_buffer(device, web::buffer(GpuBufferUsage::COPY_DST));
    gpu.gpu_buffer_drop(GpuBuffer(buffer.0));
    let error = scoped(&mut gpu, device, |gpu| {
        gpu.gpu_queue_write_buffer_with_copy(queue, GpuBuffer(buffer.0), 0, &[0; 16], None, None)
            .unwrap()
    })
    .expect("the dropped handle is refused");
    assert!(error.contains("buffer"), "{error}");
    let next = gpu.gpu_device_create_buffer(device, web::buffer(GpuBufferUsage::COPY_DST));
    assert_ne!(next.0, buffer.0);
}
