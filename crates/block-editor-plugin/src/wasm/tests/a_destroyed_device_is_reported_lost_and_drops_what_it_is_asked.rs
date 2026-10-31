use std::sync::{Arc, Mutex};

use super::*;

#[test]
fn a_destroyed_device_is_reported_lost_and_drops_what_it_is_asked() {
    let (device, queue) = gpu();
    let lost = Arc::new(Mutex::new(None));
    let told = Arc::clone(&lost);
    device.set_device_lost_callback(move |reason, _message| {
        *told.lock().unwrap() = Some(reason);
    });
    let buffer = readable(&device, 16);
    device.destroy();
    queue.write_buffer(&buffer, 0, &[1; 16]);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.clear_buffer(&buffer, 0, None);
    queue.submit([encoder.finish()]);
    let answer = map(&buffer, wgpu::MapMode::Read);
    block_gpu_guest::poll();
    assert_eq!(
        *lost.lock().unwrap(),
        Some(wgpu::DeviceLostReason::Destroyed)
    );
    assert!(
        matches!(*answer.lock().unwrap(), Some(Err(_))),
        "a map of the destroyed device's buffer fails"
    );

    let (device, queue) = gpu();
    let buffer = readable(&device, 4);
    queue.write_buffer(&buffer, 0, &[5; 4]);
    queue.submit([]);
    assert_eq!(
        read_back(&device, &buffer),
        [5; 4],
        "a device requested again works, as the host's always did"
    );
}
