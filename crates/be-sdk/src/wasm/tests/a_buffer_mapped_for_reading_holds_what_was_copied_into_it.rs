use super::*;

#[test]
fn a_buffer_mapped_for_reading_holds_what_was_copied_into_it() {
    let (device, queue) = gpu();
    let source = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("source"),
        size: 16,
        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(
        &source,
        0,
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
    );
    let destination = readable(&device, 16);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_buffer_to_buffer(&source, 4, &destination, 8, 8);
    queue.submit([encoder.finish()]);
    let answer = map(&destination, wgpu::MapMode::Read);
    assert_eq!(
        *answer.lock().unwrap(),
        None,
        "a map is answered when the plugin polls, not while it asks"
    );
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(*answer.lock().unwrap(), Some(Ok(())));
    assert_eq!(
        destination.get_mapped_range(..).to_vec(),
        [0, 0, 0, 0, 0, 0, 0, 0, 5, 6, 7, 8, 9, 10, 11, 12]
    );
    assert_eq!(
        destination.get_mapped_range(8..16).to_vec(),
        [5, 6, 7, 8, 9, 10, 11, 12],
        "a part of the mapped range reads only that part"
    );
    destination.unmap();
}
