use super::*;

#[test]
fn a_cleared_range_of_a_buffer_reads_back_as_zeros() {
    let (device, queue) = gpu();
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("cleared"),
        size: 16,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, &[7; 16]);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.clear_buffer(&buffer, 4, Some(8));
    queue.submit([encoder.finish()]);
    assert_eq!(
        read_back(&device, &buffer),
        [7, 7, 7, 7, 0, 0, 0, 0, 0, 0, 0, 0, 7, 7, 7, 7]
    );
}
