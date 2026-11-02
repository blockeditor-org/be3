use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

#[test]
fn submitted_work_calls_back_once_the_gpu_has_done_it() {
    let (device, queue) = gpu();
    let buffer = readable(&device, 16);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.clear_buffer(&buffer, 0, None);
    queue.submit([encoder.finish()]);
    let done = Arc::new(AtomicBool::new(false));
    let called = Arc::clone(&done);
    queue.on_submitted_work_done(move || called.store(true, Ordering::SeqCst));
    assert!(
        !done.load(Ordering::SeqCst),
        "the callback waits for the plugin to poll"
    );
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert!(
        done.load(Ordering::SeqCst),
        "the callback was called once the gpu had done the work"
    );
}
