use super::*;

#[test]
fn a_device_offers_only_the_features_browser_webgpu_has() {
    let (device, queue) = wgpu::Device::noop(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::SHADER_F16
            | wgpu::Features::TEXTURE_COMPRESSION_BC
            | wgpu::Features::IMMEDIATES
            | wgpu::Features::MAPPABLE_PRIMARY_BUFFERS,
        ..Default::default()
    });
    let mut gpu = Gpu::new(device, queue);
    assert_eq!(
        gpu.features(),
        [
            abi::FeatureName::TextureCompressionBc,
            abi::FeatureName::ShaderF16
        ],
        "immediate data and native-only features are not offered"
    );
}
