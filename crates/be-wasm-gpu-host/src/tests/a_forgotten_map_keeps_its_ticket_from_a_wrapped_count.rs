use be_wasm_ops::local::Promise;

use super::web::*;
use super::*;

#[test]
fn a_forgotten_map_keeps_its_ticket_from_a_wrapped_count() {
    let mut gpu = gpu();
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
    gpu.gpu_buffer_map_async(
        GpuBuffer(buffer.0),
        GpuMapMode::READ,
        None,
        None,
        Promise::new(2),
    );
    // Dropped before it settled: the map goes on, unheard, and its ticket is still the host's.
    gpu.let_go(&[2]);
    gpu.wind_tickets(u32::MAX - 1);
    assert_eq!(gpu.webgpu_ticket(), u32::MAX);
    assert_eq!(gpu.webgpu_ticket(), 1);
    assert_eq!(
        gpu.webgpu_ticket(),
        3,
        "the forgotten map's ticket is not handed out again"
    );
}
