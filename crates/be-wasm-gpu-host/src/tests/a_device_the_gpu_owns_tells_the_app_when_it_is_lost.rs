use super::web::*;
use super::*;

#[test]
fn a_device_the_gpu_owns_tells_the_app_when_it_is_lost() {
    let (gpu, wakes) = waking();
    let mut gpu = gpu.owning_device();
    let device = device(&mut gpu, Vec::new()).unwrap();
    gpu.gpu_device_lost(device, be_wasm_ops::local::Promise::new(7));
    gpu.device().destroy();
    let _ = gpu.device().poll(wgpu::PollType::wait_indefinitely());
    wakes
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("the loss woke the app");
    let lost: GpuDeviceLostInfo = settled(&mut gpu, 7);
    assert_eq!(
        gpu.gpu_device_lost_info_reason(lost),
        GpuDeviceLostReason::Unknown
    );
    assert!(!gpu.gpu_device_lost_info_message(lost).is_empty());

    let again = web::device(&mut gpu, Vec::new()).unwrap();
    gpu.gpu_device_lost(again, be_wasm_ops::local::Promise::new(8));
    let lost: GpuDeviceLostInfo = settled(&mut gpu, 8);
    assert_eq!(
        gpu.gpu_device_lost_info_reason(lost),
        GpuDeviceLostReason::Unknown,
        "a device the host lost stays lost"
    );
}
