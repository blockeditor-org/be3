use super::web::*;
use super::*;

#[test]
fn a_settled_map_wakes_the_app_without_its_polling() {
    let (mut gpu, wakes) = waking();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let buffer = gpu.gpu_device_create_buffer(
        device,
        GpuBufferDescriptor {
            size: 8,
            usage: GpuBufferUsage::MAP_READ | GpuBufferUsage::COPY_DST,
            mapped_at_creation: None,
            label: None,
        },
    );
    gpu.gpu_buffer_map_async(GpuBuffer(buffer.0), GpuMapMode::READ, None, None, be_wasm_ops::local::Promise::new(4));
    wakes
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("the settled map woke the app");
    let mapped: Result<(), MapAsyncError> = web::settled_now(&mut gpu, 4);
    assert!(mapped.is_ok());
    assert_eq!(
        gpu.gpu_buffer_get_mapped_range_get_with_copy(GpuBuffer(buffer.0), None, None)
            .map_err(|error| error.kind),
        Ok(vec![0; 8])
    );
    gpu.gpu_buffer_map_async(GpuBuffer(buffer.0), GpuMapMode::READ, None, None, be_wasm_ops::local::Promise::new(5));
    let again: Result<(), MapAsyncError> = settled(&mut gpu, 5);
    assert!(again.is_err(), "a map refused at once still settles, as failed");
    let logged = gpu.take_logged();
    assert!(
        logged.iter().any(|error| error.contains("mapped")),
        "a second map of a mapped buffer is refused: {logged:?}"
    );
}
