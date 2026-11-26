use be_wasm_ops::local::Promise;

use super::web::*;
use super::*;

#[test]
fn a_dropped_pop_of_a_waiting_scope_settles_nothing_and_keeps_no_error() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    gpu.gpu_device_push_error_scope(device, GpuErrorFilter::Validation);
    // A buffer mapped for both reading and writing is a validation error.
    gpu.gpu_device_create_buffer(
        device,
        buffer(GpuBufferUsage::MAP_READ | GpuBufferUsage::MAP_WRITE),
    );
    gpu.hold_scope(device.0, 1);
    gpu.gpu_device_pop_error_scope(device, Promise::new(4));
    let held = gpu.errors_held();
    let settled: Vec<be_wasm_ops::local::Settled> = be_wasm_gpu_abi::embed::webgpu::Host::settle(&mut gpu, false, &[4]);
    assert!(settled.is_empty(), "the pop waits on its scope: {settled:?}");
    gpu.hold_scope(device.0, -1);
    let settled: Vec<be_wasm_ops::local::Settled> = be_wasm_gpu_abi::embed::webgpu::Host::settle(&mut gpu, false, &[]);
    assert!(
        settled.iter().all(|settled| settled.ticket != 4),
        "the dropped pop settled once its scope did: {settled:?}"
    );
    assert_eq!(
        gpu.errors_held(),
        held,
        "the error the scope caught is kept for nobody"
    );
}
