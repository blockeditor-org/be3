use super::web::*;
use super::*;

#[test]
fn a_webgpu_error_goes_to_the_innermost_scope_of_its_kind() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).expect("a device with no features is given");
    gpu.gpu_device_push_error_scope(device, GpuErrorFilter::Validation);
    gpu.gpu_device_push_error_scope(device, GpuErrorFilter::OutOfMemory);
    // A buffer mapped for both reading and writing is a validation error.
    gpu.gpu_device_create_buffer(device, buffer(GpuBufferUsage::MAP_READ | GpuBufferUsage::MAP_WRITE));
    assert_eq!(pop(&mut gpu, device, 1), Ok(None), "an out-of-memory scope does not catch it");
    let caught = pop(&mut gpu, device, 2)
        .expect("a scope was open")
        .expect("the validation scope caught the error");
    assert_eq!(gpu.gpu_error_kind(caught), GpuErrorKind::ValidationError);
    assert!(!gpu.gpu_error_message(caught).is_empty());
    assert_eq!(
        pop(&mut gpu, device, 3).map_err(|error| error.kind),
        Err(PopErrorScopeErrorKind::OperationError),
        "a pop with no scope open is refused"
    );
    assert!(gpu.take_logged().is_empty());
    assert_eq!(gpu.take_error(), None, "be3:gpu/device's error is not touched");
}
