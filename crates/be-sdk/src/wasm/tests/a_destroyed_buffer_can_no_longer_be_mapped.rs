use super::*;

#[test]
fn a_destroyed_buffer_can_no_longer_be_mapped() {
    let (device, _queue) = gpu();
    let buffer = readable(&device, 16);
    buffer.destroy();
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let answer = map(&buffer, wgpu::MapMode::Read);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let popped = pin!(scope.pop()).poll(&mut Context::from_waker(Waker::noop()));
    assert!(
        matches!(popped, Poll::Ready(Some(wgpu::Error::Validation { .. }))),
        "mapping the destroyed buffer was not refused"
    );
    assert!(
        matches!(*answer.lock().unwrap(), Some(Err(_))),
        "the destroyed buffer's map did not fail"
    );
}
