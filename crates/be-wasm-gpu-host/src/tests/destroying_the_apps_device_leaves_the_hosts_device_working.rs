use super::web::*;
use super::*;

#[test]
fn destroying_the_apps_device_leaves_the_hosts_device_working() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let queue = gpu.gpu_device_queue(device);
    let buffer = gpu.gpu_device_create_buffer(device, web::buffer(GpuBufferUsage::COPY_DST));
    gpu.gpu_device_destroy(device);
    gpu.gpu_queue_write_buffer_with_copy(queue, GpuBuffer(buffer.0), 0, &[1; 4], None, None)
        .unwrap();
    let encoder = gpu.gpu_device_create_command_encoder(device, None);
    gpu.gpu_command_encoder_clear_buffer(encoder, GpuBuffer(buffer.0), None, None);
    let commands = gpu.gpu_command_encoder_finish(encoder, None);
    gpu.gpu_queue_submit(queue, &[commands]);
    assert!(
        gpu.take_logged().is_empty(),
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
    assert_eq!(crate::caught(scope.pop()), None, "the host's device still works");

    let again = web::device(&mut gpu, Vec::new()).unwrap();
    let queue = gpu.gpu_device_queue(again);
    let buffer = gpu.gpu_device_create_buffer(again, web::buffer(GpuBufferUsage::COPY_DST));
    let written = scoped(&mut gpu, again, |gpu| {
        gpu.gpu_queue_write_buffer_with_copy(queue, GpuBuffer(buffer.0), 0, &[1; 4], None, None)
            .unwrap()
    });
    assert_eq!(written, None, "a device asked for again works");
}
