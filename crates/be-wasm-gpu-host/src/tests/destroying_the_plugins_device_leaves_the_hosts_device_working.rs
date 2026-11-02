use super::*;

#[test]
fn destroying_the_plugins_device_leaves_the_hosts_device_working() {
    let mut gpu = gpu();
    let buffer = gpu.create_buffer(buffer_descriptor()).unwrap();
    gpu.destroy_device();
    let lost = gpu.device_lost().expect("the plugin's device is lost");
    assert_eq!(lost.reason, abi::DeviceLostReason::Destroyed);
    gpu.write_buffer(buffer, 0, &[1; 4]);
    let encoder = gpu
        .create_command_encoder(abi::CommandEncoderDescriptor { label: None })
        .expect("a destroyed device still answers a handle");
    gpu.command_encoder_clear_buffer(encoder, buffer, 0, None);
    let commands = gpu.command_encoder_finish(encoder).unwrap();
    gpu.submit(&[commands]);
    assert_eq!(
        gpu.take_error(),
        None,
        "what a destroyed device is asked is dropped without errors"
    );

    let scope = gpu.device().push_error_scope(wgpu::ErrorFilter::Validation);
    let shared = gpu.device().create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 4,
        usage: wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu.queue().write_buffer(&shared, 0, &[1; 4]);
    assert_eq!(caught(scope.pop()), None, "the host's device still works");

    gpu.request_device();
    assert_eq!(gpu.device_lost(), None);
    let buffer = gpu.create_buffer(buffer_descriptor()).unwrap();
    gpu.write_buffer(buffer, 0, &[1; 4]);
    assert_eq!(gpu.take_error(), None);
}
