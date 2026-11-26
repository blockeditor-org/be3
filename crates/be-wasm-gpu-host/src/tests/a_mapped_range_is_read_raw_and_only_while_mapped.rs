use super::web::*;
use super::*;

/// The bytes of a read the guest was answered, as its encoded `result`.
fn read(encoded: &[u8]) -> Option<Vec<u8>> {
    be_wasm_ops::decode::<Result<Vec<u8>, GetMappedRangeError>>(encoded)?.ok()
}

#[test]
fn a_mapped_range_is_read_raw_and_only_while_mapped() {
    let mut guest = Guest::new();
    let device = device(&mut guest.gpu, Vec::new()).unwrap();
    let buffer = guest.gpu.gpu_device_create_buffer(
        device,
        GpuBufferDescriptor {
            size: 16,
            usage: GpuBufferUsage::MAP_WRITE | GpuBufferUsage::COPY_SRC,
            mapped_at_creation: Some(true),
            label: None,
        },
    );
    let written: Vec<u8> = (1..=16).collect();
    guest
        .gpu
        .gpu_buffer_get_mapped_range_set_with_copy(GpuBuffer(buffer.0), &written, Some(0), Some(16))
        .unwrap();
    let length = glue::gpu_buffer_get_mapped_range_get_with_copy(&mut guest, buffer.0, 1, 0, 1, 8, 64, 64);
    assert_eq!(length, 10, "the read is answered, its encoding's length");
    glue::returned(&mut guest, 64);
    assert_eq!(
        read(&guest.memory[64..74]),
        Some(vec![1, 2, 3, 4, 5, 6, 7, 8])
    );
    let length = glue::gpu_buffer_get_mapped_range_get_with_copy(&mut guest, buffer.0, 1, 8, 1, 4, 0, 0);
    assert_eq!(length, 6, "a read waits to be taken");
    glue::returned(&mut guest, 128);
    assert_eq!(
        read(&guest.memory[128..134]),
        Some(vec![9, 10, 11, 12])
    );
    let misaligned = guest
        .gpu
        .gpu_buffer_get_mapped_range_get_with_copy(GpuBuffer(buffer.0), Some(2), Some(4));
    assert!(misaligned.is_err(), "a misaligned read is refused");
    guest.gpu.gpu_buffer_unmap(GpuBuffer(buffer.0)).unwrap();
    let unmapped = guest
        .gpu
        .gpu_buffer_get_mapped_range_get_with_copy(GpuBuffer(buffer.0), Some(0), Some(8));
    assert!(unmapped.is_err(), "a read of an unmapped buffer is refused");
    assert!(guest.refused.is_empty(), "{:?}", guest.refused);
}
