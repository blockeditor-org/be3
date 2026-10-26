use super::*;

#[test]
fn a_created_buffer_gets_a_live_handle() {
    let mut gpu = gpu();
    let buffer = gpu
        .create_buffer(buffer_descriptor())
        .expect("the buffer should be created");
    assert_ne!(buffer.0, 0);
    assert_eq!(gpu.take_error(), None);
    gpu.write_buffer(buffer, 0, &[0u8; 16]);
    assert_eq!(gpu.take_error(), None);
}
