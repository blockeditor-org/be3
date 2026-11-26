use be_wasm_ops::local::Promise;

use super::web::*;
use super::*;

#[test]
fn webgpu_maps_and_work_settle_through_the_poll() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let queue = gpu.gpu_device_queue(device);
    let buffer = gpu.gpu_device_create_buffer(
        device,
        buffer(GpuBufferUsage::MAP_READ | GpuBufferUsage::COPY_DST),
    );
    assert_eq!(
        gpu.gpu_buffer_map_state(buffer),
        GpuBufferMapState::Unmapped
    );
    gpu.gpu_queue_write_buffer_with_copy(queue, buffer, 0, &[1, 2, 3, 4], None, None)
        .expect("the data covers what was asked");
    gpu.gpu_queue_on_submitted_work_done(queue, Promise::new(3));
    let () = settled(&mut gpu, 3);

    gpu.gpu_buffer_map_async(buffer, GpuMapMode::READ, None, Some(4), Promise::new(4));
    let mapped: Result<(), MapAsyncError> = settled(&mut gpu, 4);
    assert_eq!(mapped.map_err(|error| error.kind), Ok(()));
    assert_eq!(gpu.gpu_buffer_map_state(buffer), GpuBufferMapState::Mapped);
    assert_eq!(
        gpu.gpu_buffer_get_mapped_range_get_with_copy(buffer, Some(0), Some(4))
            .map_err(|error| error.kind),
        Ok(vec![1, 2, 3, 4])
    );
    gpu.gpu_buffer_unmap(buffer).unwrap();
    assert_eq!(
        gpu.gpu_buffer_map_state(buffer),
        GpuBufferMapState::Unmapped
    );
}
