use super::web::*;
use super::*;

#[test]
fn a_compute_pass_is_recorded_on_its_encoder() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let module = module(&mut gpu, device, "@compute @workgroup_size(1) fn main() {}");
    let pipeline = gpu.gpu_device_create_compute_pipeline(
        device,
        GpuComputePipelineDescriptor {
            compute: GpuProgrammableStage {
                module,
                entry_point: None,
                constants: None,
            },
            layout: GpuLayoutMode::Auto,
            label: Some("nothing".into()),
        },
    );
    let queue = gpu.gpu_device_queue(device);
    let mut pass = None;
    let recorded = scoped(&mut gpu, device, |gpu| {
        let encoder = gpu.gpu_device_create_command_encoder(device, None);
        let begun = gpu.gpu_command_encoder_begin_compute_pass(encoder, None);
        gpu.gpu_compute_pass_encoder_set_pipeline(begun, pipeline);
        gpu.gpu_compute_pass_encoder_dispatch_workgroups(begun, 2, None, None);
        gpu.gpu_compute_pass_encoder_end(begun);
        let commands = gpu.gpu_command_encoder_finish(encoder, None);
        gpu.gpu_queue_submit(queue, &[commands]);
        pass = Some(begun);
    });
    assert_eq!(recorded, None);
    let pass = pass.unwrap();
    let error = scoped(&mut gpu, device, |gpu| {
        gpu.gpu_compute_pass_encoder_dispatch_workgroups(pass, 1, None, None)
    })
    .expect("a dispatch on an ended pass is refused");
    assert!(error.contains("compute pass"), "{error}");
}
