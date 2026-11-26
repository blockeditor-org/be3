use super::web::*;
use super::*;

#[test]
fn every_webgpu_limit_of_the_device_is_forwarded() {
    let required = wgpu::Limits {
        max_color_attachments: 7,
        max_compute_workgroup_size_x: 128,
        max_inter_stage_shader_variables: 15,
        max_storage_buffers_per_shader_stage: 6,
        ..wgpu::Limits::downlevel_defaults()
    };
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor {
        required_limits: required,
        ..Default::default()
    });
    let mut gpu = Gpu::new(device, queue);
    let instance = gpu.get_gpu();
    gpu.gpu_request_adapter(instance, None, be_wasm_ops::local::Promise::new(TICKET));
    let adapter: Option<GpuAdapter> = settled(&mut gpu, TICKET);
    let limits = gpu.gpu_adapter_limits(adapter.unwrap());
    let device = gpu.device().limits();
    assert_eq!(
        (
            gpu.gpu_supported_limits_max_color_attachments(limits),
            gpu.gpu_supported_limits_max_compute_workgroup_size_x(limits),
            gpu.gpu_supported_limits_max_inter_stage_shader_variables(limits),
            gpu.gpu_supported_limits_max_storage_buffers_per_shader_stage(limits),
            gpu.gpu_supported_limits_max_texture_dimension2_d(limits),
        ),
        (
            device.max_color_attachments,
            device.max_compute_workgroup_size_x,
            device.max_inter_stage_shader_variables,
            device.max_storage_buffers_per_shader_stage,
            device.max_texture_dimension_2d,
        ),
    );
}
