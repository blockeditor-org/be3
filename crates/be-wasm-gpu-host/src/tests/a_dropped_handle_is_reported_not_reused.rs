use super::*;

#[test]
fn a_dropped_handle_is_reported_not_reused() {
    let mut gpu = gpu();
    let buffer = gpu.create_buffer(buffer_descriptor()).unwrap();
    gpu.buffer_drop(buffer);
    gpu.write_buffer(buffer, 0, &[0u8; 16]);
    let error = gpu.take_error().expect("the dropped handle should report");
    assert!(error.contains("buffer"), "{error}");
    let next = gpu.create_buffer(buffer_descriptor()).unwrap();
    assert_ne!(next, buffer);
}
