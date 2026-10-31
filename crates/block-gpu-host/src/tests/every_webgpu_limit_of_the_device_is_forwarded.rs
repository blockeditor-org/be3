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
        required_limits: required.clone(),
        ..Default::default()
    });
    let mut gpu = Gpu::new(device, queue);
    let limits = gpu.limits();
    let device = gpu.device().limits();
    assert_eq!(
        (
            limits.max_color_attachments,
            limits.max_compute_workgroup_size_x,
            limits.max_inter_stage_shader_variables,
            limits.max_storage_buffers_per_shader_stage,
            limits.max_texture_dimension2d,
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
