use super::*;

#[test]
fn a_compute_pass_is_recorded_on_its_encoder() {
    let mut gpu = gpu();
    let module = gpu
        .create_shader_module(abi::ShaderModuleDescriptor {
            label: None,
            wgsl: "@compute @workgroup_size(1) fn main() {}".into(),
        })
        .unwrap();
    let pipeline = gpu
        .create_compute_pipeline(abi::ComputePipelineDescriptor {
            label: Some("nothing".into()),
            layout: None,
            module,
            entry_point: None,
        })
        .unwrap();
    let encoder = gpu
        .create_command_encoder(abi::CommandEncoderDescriptor { label: None })
        .unwrap();
    let pass = gpu
        .command_encoder_begin_compute_pass(
            encoder,
            abi::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            },
        )
        .unwrap();
    gpu.compute_pass_set_pipeline(pass, pipeline);
    gpu.compute_pass_dispatch_workgroups(pass, 2, 1, 1);
    gpu.compute_pass_end(pass);
    let buffer = gpu.command_encoder_finish(encoder).unwrap();
    gpu.submit(&[buffer]);
    assert_eq!(gpu.take_error(), None);
    gpu.compute_pass_dispatch_workgroups(pass, 1, 1, 1);
    let error = gpu
        .take_error()
        .expect("a dispatch on an ended pass should report");
    assert!(error.contains("compute pass"), "{error}");
}
