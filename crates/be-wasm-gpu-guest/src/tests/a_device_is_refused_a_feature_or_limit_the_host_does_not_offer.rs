use super::*;

#[test]
fn a_device_is_refused_a_feature_or_limit_the_host_does_not_offer() {
    let offered = wgpu::Features::SHADER_F16;
    let limits = wgpu::Limits::default();
    let asking = |required_features, required_limits| {
        requested(
            &wgpu::DeviceDescriptor {
                required_features,
                required_limits,
                ..Default::default()
            },
            offered,
            &limits,
        )
    };
    assert_eq!(asking(offered, limits.clone()), Ok(()));
    assert!(
        asking(wgpu::Features::TEXTURE_COMPRESSION_BC, limits.clone()).is_err(),
        "a feature the host does not offer is refused"
    );
    let larger = wgpu::Limits {
        max_texture_dimension_2d: limits.max_texture_dimension_2d * 2,
        ..limits.clone()
    };
    assert!(
        asking(wgpu::Features::empty(), larger).is_err(),
        "a limit better than the host's is refused"
    );
}
