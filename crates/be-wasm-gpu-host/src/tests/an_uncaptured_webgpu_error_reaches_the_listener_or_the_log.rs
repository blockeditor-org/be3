use be_wasm_ops::local::Sender;

use super::web::*;
use super::*;

#[test]
fn an_uncaptured_webgpu_error_reaches_the_listener_or_the_log() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).expect("a device with no features is given");
    let invalid = || buffer(GpuBufferUsage::MAP_READ | GpuBufferUsage::MAP_WRITE);
    gpu.gpu_device_create_buffer(device, invalid());
    let logged = gpu.take_logged();
    assert_eq!(logged.len(), 1, "with no listener it goes to the app's log: {logged:?}");

    gpu.gpu_device_on_uncaptured_error(device, Sender::new(7));
    gpu.gpu_device_create_buffer(device, invalid());
    let error: GpuError = settled(&mut gpu, 7);
    assert_eq!(gpu.gpu_error_kind(error), GpuErrorKind::ValidationError);
    assert!(gpu.take_logged().is_empty());
}
