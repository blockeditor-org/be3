use super::web::*;
use super::*;

#[test]
fn a_webgpu_device_has_only_what_it_asked_for() {
    let mut gpu = gpu();
    let refused = device(&mut gpu, vec![GpuFeatureName::TimestampQuery]);
    assert_eq!(
        refused.map_err(|error| error.kind),
        Err(RequestDeviceErrorKind::TypeError),
        "the noop adapter has no timestamp queries"
    );
    let device = device(&mut gpu, Vec::new()).expect("a device with no features is given");
    let features = gpu.gpu_device_features(device);
    assert!(gpu.gpu_supported_features_has(features, "core-features-and-limits"));
    assert!(!gpu.gpu_supported_features_has(features, "timestamp-query"));
    let set = gpu.gpu_device_create_query_set(
        device,
        GpuQuerySetDescriptor {
            r#type: GpuQueryType::Timestamp,
            count: 2,
            label: None,
        },
    );
    assert_eq!(
        set.map_err(|error| error.kind),
        Err(CreateQuerySetErrorKind::TypeError)
    );
    let limits = gpu.gpu_device_limits(device);
    assert_eq!(
        gpu.gpu_supported_limits_max_bind_groups(limits),
        wgpu::Limits::default().max_bind_groups,
        "a device asking for no limits has the defaults"
    );
}
