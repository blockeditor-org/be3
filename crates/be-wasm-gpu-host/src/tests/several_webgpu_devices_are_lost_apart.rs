use be_wasm_ops::local::Promise;

use super::web::*;
use super::*;

#[test]
fn several_webgpu_devices_are_lost_apart() {
    let mut gpu = gpu();
    let first = device(&mut gpu, Vec::new()).unwrap();
    let second = device(&mut gpu, Vec::new()).unwrap();
    gpu.gpu_device_lost(first, Promise::new(5));
    let kept = gpu.gpu_device_create_buffer(first, buffer(GpuBufferUsage::COPY_DST));
    gpu.gpu_device_destroy(first);
    let lost: GpuDeviceLostInfo = settled(&mut gpu, 5);
    assert_eq!(gpu.gpu_device_lost_info_reason(lost), GpuDeviceLostReason::Destroyed);
    assert!(
        gpu.buffers.get(kept.0, "buffer").is_ok(),
        "its buffer is destroyed, not forgotten"
    );

    gpu.gpu_device_push_error_scope(second, GpuErrorFilter::Validation);
    let made = gpu.gpu_device_create_buffer(second, buffer(GpuBufferUsage::COPY_DST));
    assert!(gpu.buffers.get(made.0, "buffer").is_ok(), "the other device goes on");
    assert_eq!(pop(&mut gpu, second, 6), Ok(None));

    let invalid = gpu.gpu_device_create_buffer(first, buffer(GpuBufferUsage::COPY_DST));
    assert!(
        gpu.buffers.get(invalid.0, "buffer").is_err(),
        "a lost device makes invalid objects"
    );
    assert!(gpu.take_logged().is_empty(), "and raises no errors");
}
