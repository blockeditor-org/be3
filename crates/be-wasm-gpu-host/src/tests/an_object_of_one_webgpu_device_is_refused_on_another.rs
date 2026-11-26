use super::web::*;
use super::*;

#[test]
fn an_object_of_one_webgpu_device_is_refused_on_another() {
    let mut gpu = gpu();
    let first = device(&mut gpu, Vec::new()).unwrap();
    let second = device(&mut gpu, Vec::new()).unwrap();
    let theirs = gpu.gpu_device_create_buffer(first, buffer(GpuBufferUsage::COPY_DST));
    let own = gpu.gpu_device_create_buffer(second, buffer(GpuBufferUsage::COPY_DST));
    let queue = gpu.gpu_device_queue(second);

    gpu.gpu_device_push_error_scope(second, GpuErrorFilter::Validation);
    gpu.gpu_queue_write_buffer_with_copy(queue, GpuBuffer(own.0), 0, &[0; 4], None, None)
        .unwrap();
    assert_eq!(
        pop(&mut gpu, second, 7),
        Ok(None),
        "its own buffer is written"
    );

    gpu.gpu_device_push_error_scope(second, GpuErrorFilter::Validation);
    gpu.gpu_queue_write_buffer_with_copy(queue, GpuBuffer(theirs.0), 0, &[0; 4], None, None)
        .unwrap();
    let encoder = gpu.gpu_device_create_command_encoder(second, None);
    gpu.gpu_command_encoder_clear_buffer(encoder, GpuBuffer(theirs.0), None, None);
    let caught = pop(&mut gpu, second, 8)
        .expect("the scope pops")
        .expect("another device's buffer is refused");
    let message = gpu.gpu_error_message(caught);
    assert!(
        message.contains("another device"),
        "it was refused as {message}"
    );
}
