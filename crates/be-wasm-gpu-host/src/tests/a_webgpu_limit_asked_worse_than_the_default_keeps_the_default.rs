use be_wasm_ops::local::Promise;

use super::web::*;
use super::*;

/// A device asking for `limits`, by WebGPU's names.
fn asking(gpu: &mut Gpu, limits: &[(&str, u64)]) -> Result<GpuDevice, RequestDeviceError> {
    let instance = gpu.get_gpu();
    gpu.gpu_request_adapter(instance, None, Promise::new(TICKET));
    let adapter: Option<GpuAdapter> = settled(gpu, TICKET);
    let record = gpu.record_option_gpu_size64_new();
    for (name, value) in limits {
        gpu.record_option_gpu_size64_add(RecordOptionGpuSize64(record.0), name, Some(*value));
    }
    let descriptor = GpuDeviceDescriptor {
        required_features: None,
        required_limits: Some(RecordOptionGpuSize64(record.0)),
        default_queue: None,
        label: None,
    };
    gpu.gpu_adapter_request_device(adapter.unwrap(), Some(descriptor), Promise::new(TICKET + 1));
    settled(gpu, TICKET + 1)
}

#[test]
fn a_webgpu_limit_asked_worse_than_the_default_keeps_the_default() {
    let mut gpu = gpu();
    let defaults = wgpu::Limits::default();
    let device = asking(
        &mut gpu,
        &[("maxBindGroups", 1), ("minUniformBufferOffsetAlignment", 4096)],
    )
    .expect("a worse limit is no reason to refuse a device");
    let limits = gpu.gpu_device_limits(device);
    assert_eq!(gpu.gpu_supported_limits_max_bind_groups(limits), defaults.max_bind_groups);
    assert_eq!(
        gpu.gpu_supported_limits_min_uniform_buffer_offset_alignment(limits),
        defaults.min_uniform_buffer_offset_alignment
    );

    let refused = asking(&mut gpu, &[("minStorageBufferOffsetAlignment", 300)]);
    assert_eq!(
        refused.map_err(|error| error.kind),
        Err(RequestDeviceErrorKind::OperationError),
        "an alignment is a power of two"
    );

    gpu.gpu_device_push_error_scope(device, GpuErrorFilter::Validation);
    let _layers = gpu.gpu_device_create_texture(
        device,
        GpuTextureDescriptor {
            size: GpuExtent3D {
                width: 4,
                height: Some(4),
                depth_or_array_layers: Some(defaults.max_texture_array_layers + 1),
            },
            mip_level_count: None,
            sample_count: None,
            dimension: None,
            format: GpuTextureFormat::Rgba8unorm,
            usage: GpuTextureUsage::TEXTURE_BINDING,
            view_formats: None,
            texture_binding_view_dimension: None,
            label: None,
        },
    );
    let caught = pop(&mut gpu, device, TICKET + 2)
        .expect("the scope pops")
        .expect("a texture with more layers than the device allows is refused");
    let message = gpu.gpu_error_message(caught);
    assert!(message.contains("array layers"), "it was refused as {message}");
}
