use super::web::*;
use super::*;

#[test]
fn an_async_pipeline_is_made_off_the_apps_thread_and_settles_through_the_poll() {
    let (mut gpu, wakes) = waking();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let module = module(&mut gpu, device, "@compute @workgroup_size(1) fn main() {}");
    let descriptor = |entry_point: &str| GpuComputePipelineDescriptor {
        compute: GpuProgrammableStage {
            module,
            entry_point: Some(entry_point.to_owned()),
            constants: None,
        },
        layout: GpuLayoutMode::Auto,
        label: None,
    };
    gpu.gpu_device_create_compute_pipeline_async(
        device,
        descriptor("main"),
        be_wasm_ops::local::Promise::new(4),
    );
    wakes
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("the made pipeline woke the app");
    let made: Result<GpuComputePipeline, CreatePipelineError> = settled(&mut gpu, 4);
    let made = made.expect("the pipeline was made");
    // The app's to use: asking it for its layout raises nothing.
    gpu.gpu_compute_pipeline_get_bind_group_layout(made, 0);

    gpu.gpu_device_create_compute_pipeline_async(
        device,
        descriptor("missing"),
        be_wasm_ops::local::Promise::new(5),
    );
    let refused: Result<GpuComputePipeline, CreatePipelineError> = settled(&mut gpu, 5);
    let refused = refused.expect_err("a pipeline with no such entry point was made");
    assert!(
        refused.message.contains("missing"),
        "it was refused as {}",
        refused.message
    );
    assert_eq!(
        gpu.take_logged(),
        Vec::<String>::new(),
        "the made pipeline is the app's, and a refused one raises nothing"
    );
}
