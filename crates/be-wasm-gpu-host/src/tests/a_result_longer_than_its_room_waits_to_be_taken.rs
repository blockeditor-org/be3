use super::web::*;
use super::*;

#[test]
fn a_result_longer_than_its_room_waits_to_be_taken() {
    let mut guest = Guest::new();
    let device = device(&mut guest.gpu, Vec::new()).unwrap();
    let label = "a buffer named at length ".repeat(8);
    let buffer = guest.gpu.gpu_device_create_buffer(
        device,
        GpuBufferDescriptor {
            label: Some(label.clone()),
            ..web::buffer(GpuBufferUsage::COPY_DST)
        },
    );
    let length = glue::gpu_buffer_label(&mut guest, buffer.0, 64, 4);
    assert!(length > 4);
    assert_eq!(guest.memory[64..68], [0; 4]);
    glue::returned(&mut guest, 128);
    let bytes = &guest.memory[128..128 + length as usize];
    assert_eq!(be_wasm_ops::decode::<String>(bytes), Some(label));
    assert!(guest.refused.is_empty());
}
