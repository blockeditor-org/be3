use super::*;

#[test]
fn a_buffer_mapped_for_writing_keeps_what_was_written() {
    let (device, queue) = gpu();
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("staging"),
        size: 8,
        usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let answer = map(&staging, wgpu::MapMode::Write);
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(*answer.lock().unwrap(), Some(Ok(())));
    staging
        .get_mapped_range_mut(..)
        .copy_from_slice(&[9, 8, 7, 6, 5, 4, 3, 2]);
    staging.unmap();
    let destination = readable(&device, 8);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_buffer_to_buffer(&staging, 0, &destination, 0, None);
    queue.submit([encoder.finish()]);
    assert_eq!(read_back(&device, &destination), [9, 8, 7, 6, 5, 4, 3, 2]);
}
