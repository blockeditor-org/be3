use super::*;

#[test]
fn a_map_still_pending_when_its_buffer_is_unmapped_fails() {
    let (device, _queue) = gpu();
    let buffer = readable(&device, 8);
    let answer = map(&buffer, wgpu::MapMode::Read);
    buffer.unmap();
    assert_eq!(*answer.lock().unwrap(), Some(Err(wgpu::BufferAsyncError)));
    let dropped = readable(&device, 8);
    let answer = map(&dropped, wgpu::MapMode::Read);
    drop(dropped);
    assert_eq!(
        *answer.lock().unwrap(),
        Some(Err(wgpu::BufferAsyncError)),
        "dropping a buffer fails its pending map"
    );
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(read_back(&device, &buffer), [0; 8], "the buffer maps again");
}
