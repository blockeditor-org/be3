use super::web::*;
use super::*;

#[test]
fn a_dropped_ticket_settles_nothing_and_is_not_waited_for() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let module = module(&mut gpu, device, "@compute @workgroup_size(1) fn main() {}");
    gpu.gpu_device_create_compute_pipeline_async(
        device,
        GpuComputePipelineDescriptor {
            compute: GpuProgrammableStage {
                module,
                entry_point: None,
                constants: None,
            },
            layout: GpuLayoutMode::Auto,
            label: None,
        },
        be_wasm_ops::local::Promise::new(4),
    );
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
        be_wasm_ops::local::Promise::new(5),
    );
    // A waiting poll that is told both were dropped answers without them: it waits for the map
    // to finish, as a map dropped in WebGPU still maps its buffer, but not for the pipeline.
    let settled: Vec<be_wasm_ops::local::Settled> = be_wasm_gpu_abi::embed::webgpu::Host::settle(&mut gpu, true, &[4, 5]);
    assert!(
        settled.iter().all(|settled| ![4, 5].contains(&settled.ticket)),
        "a dropped ticket settled: {settled:?}"
    );
    let later: Vec<be_wasm_ops::local::Settled> = be_wasm_gpu_abi::embed::webgpu::Host::settle(&mut gpu, true, &[]);
    assert_eq!(later, Vec::new(), "nothing is left to settle, nor to wait for");
    assert_eq!(
        gpu.gpu_buffer_get_mapped_range_get_with_copy(GpuBuffer(buffer.0), None, None)
            .map_err(|error| error.kind),
        Ok(vec![0; 8]),
        "the dropped map still mapped its buffer"
    );
}
