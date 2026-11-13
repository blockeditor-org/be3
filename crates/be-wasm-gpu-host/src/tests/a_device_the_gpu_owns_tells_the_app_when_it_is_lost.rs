use std::sync::mpsc;
use std::time::Duration;

use super::*;

#[test]
fn a_device_the_gpu_owns_tells_the_app_when_it_is_lost() {
    let (woken, wakes) = mpsc::channel();
    let woken = Mutex::new(woken);
    let mut gpu = gpu()
        .waking(Arc::new(move || {
            let _ = woken.lock().unwrap().send(());
        }))
        .owning_device();
    assert_eq!(gpu.poll(false).lost, None);
    gpu.device().destroy();
    let _ = gpu.device().poll(wgpu::PollType::wait_indefinitely());
    wakes
        .recv_timeout(Duration::from_secs(30))
        .expect("the loss woke the app");
    let lost = gpu.poll(false).lost.expect("the device is lost");
    assert_eq!(lost.reason, abi::DeviceLostReason::Unknown);
    assert!(!lost.message.is_empty());
    gpu.request_device();
    assert!(
        gpu.poll(false).lost.is_some(),
        "a device the host lost stays lost"
    );
}
