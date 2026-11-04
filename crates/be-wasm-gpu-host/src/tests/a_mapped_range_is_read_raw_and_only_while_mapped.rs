use super::*;

#[test]
fn a_mapped_range_is_read_raw_and_only_while_mapped() {
    let mut guest = Guest::new();
    let buffer = guest
        .gpu
        .create_buffer(abi::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::MAP_WRITE.bits() | wgpu::BufferUsages::COPY_SRC.bits(),
            mapped_at_creation: true,
        })
        .unwrap();
    assert_eq!(guest.gpu.buffer_map_state(buffer), abi::MapState::Mapped);
    guest.gpu.buffer_write_mapped(
        buffer,
        0,
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
    );
    let length = glue::buffer_read_mapped(&mut guest, buffer.0, 0, 8, 64, 8);
    assert_eq!(length, 8);
    assert_eq!(guest.memory[64..72], [1, 2, 3, 4, 5, 6, 7, 8]);
    let length = glue::buffer_read_mapped(&mut guest, buffer.0, 8, 4, 0, 0);
    assert_eq!(length, 4, "a read with no room waits to be taken");
    glue::returned(&mut guest, 128);
    assert_eq!(guest.memory[128..132], [9, 10, 11, 12]);
    assert_eq!(guest.gpu.take_error(), None);
    guest.gpu.buffer_read_mapped(buffer, 2, 4);
    let error = guest.gpu.take_error().expect("a misaligned read reports");
    assert!(error.contains("multiple"), "{error}");
    guest.gpu.buffer_unmap(buffer);
    assert_eq!(guest.gpu.buffer_map_state(buffer), abi::MapState::Unmapped);
    assert_eq!(guest.gpu.buffer_read_mapped(buffer, 0, 8), Vec::<u8>::new());
    let error = guest
        .gpu
        .take_error()
        .expect("a read of an unmapped buffer reports");
    assert!(error.contains("not mapped"), "{error}");
    assert!(guest.refused.is_empty());
}
