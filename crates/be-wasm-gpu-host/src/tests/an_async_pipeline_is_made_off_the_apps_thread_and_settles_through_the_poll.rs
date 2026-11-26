use super::web::*;
use super::*;

type Made = Result<GpuComputePipeline, CreatePipelineError>;

fn compute(module: GpuShaderModule, entry_point: &str) -> GpuComputePipelineDescriptor {
    GpuComputePipelineDescriptor {
        compute: GpuProgrammableStage {
            module,
            entry_point: Some(entry_point.to_owned()),
            constants: None,
        },
        layout: GpuLayoutMode::Auto,
        label: None,
    }
}

#[test]
fn an_async_pipeline_is_made_off_the_apps_thread_and_settles_through_the_poll() {
    let (mut gpu, wakes) = waking();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let module = module(&mut gpu, device, "@compute @workgroup_size(1) fn main() {}");
    // Only the pipeline's wake may answer the wait below, not one from asking for the device.
    while wakes.try_recv().is_ok() {}
    gpu.gpu_device_create_compute_pipeline_async(
        device,
        compute(module, "main"),
        be_wasm_ops::local::Promise::new(4),
    );
    wakes
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("the made pipeline woke the app");
    let made: Made = settled_now(&mut gpu, 4);
    let made = made.expect("the pipeline was made");
    // The app's to use: asking it for its layout raises nothing.
    gpu.gpu_compute_pipeline_get_bind_group_layout(made, 0);

    // Two at once, one refused, settle in whichever order they are made.
    gpu.gpu_device_create_compute_pipeline_async(
        device,
        compute(module, "missing"),
        be_wasm_ops::local::Promise::new(5),
    );
    gpu.gpu_device_create_compute_pipeline_async(
        device,
        compute(module, "main"),
        be_wasm_ops::local::Promise::new(6),
    );
    let mut answers: Vec<be_wasm_ops::local::Settled> = Vec::new();
    while answers.len() < 2 {
        answers.extend(be_wasm_gpu_abi::embed::webgpu::Host::settle(
            &mut gpu,
            true,
            &[],
        ));
    }
    let answer = |ticket: u32| -> Made {
        let settled = answers
            .iter()
            .find(|settled| settled.ticket == ticket)
            .and_then(|settled| settled.value.as_ref())
            .unwrap_or_else(|| panic!("ticket {ticket} did not settle"));
        be_wasm_ops::decode(settled).expect("what settled decodes")
    };
    let refused = answer(5).expect_err("a pipeline with no such entry point was made");
    assert!(
        matches!(
            refused.kind,
            CreatePipelineErrorKind::GpuPipelineError(GpuPipelineErrorReason::Validation)
        ),
        "a missing entry point is a validation error"
    );
    assert!(
        refused.message.contains("missing"),
        "it was refused as {}",
        refused.message
    );
    assert!(answer(6).is_ok(), "the other was made beside it");
    assert_eq!(
        gpu.take_logged(),
        Vec::<String>::new(),
        "the made pipelines are the app's, and a refused one raises nothing"
    );

    // One being made as its device is destroyed is refused as internal.
    gpu.gpu_device_create_compute_pipeline_async(
        device,
        compute(module, "main"),
        be_wasm_ops::local::Promise::new(7),
    );
    gpu.gpu_device_destroy(device);
    let lost: Made = settled(&mut gpu, 7);
    assert!(
        matches!(
            lost.map_err(|error| error.kind),
            Err(CreatePipelineErrorKind::GpuPipelineError(
                GpuPipelineErrorReason::Internal
            ))
        ),
        "a pipeline whose device was lost is refused as internal"
    );
}
