#[test]
fn a_plugin_device_has_only_the_features_and_limits_it_requested() {
    let limits = block_gpu_guest::limits();
    let (device, _queue) = block_gpu_guest::request_device(&wgpu::DeviceDescriptor {
        required_limits: limits.clone(),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(device.features(), wgpu::Features::empty());
    assert_eq!(device.limits(), limits);
    assert!(
        (block_gpu_guest::features() & wgpu::Features::all_native_mask()).is_empty(),
        "the host offers only features browser WebGPU has"
    );
    let native = block_gpu_guest::request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::MAPPABLE_PRIMARY_BUFFERS,
        ..Default::default()
    });
    assert!(native.is_err(), "a native-only feature is never offered");
}
