use super::web::*;
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
    let instance = gpu.get_gpu();
    gpu.gpu_request_adapter(instance, None, be_wasm_ops::local::Promise::new(TICKET));
    let adapter: Option<GpuAdapter> = settled(&mut gpu, TICKET);
    let features = gpu.gpu_adapter_features(adapter.unwrap());
    for offered in [
        "core-features-and-limits",
        "texture-compression-bc",
        "shader-f16",
    ] {
        assert!(
            gpu.gpu_supported_features_has(features, offered),
            "{offered} is offered"
        );
    }
    for native in ["immediates", "mappable-primary-buffers", "timestamp-query"] {
        assert!(
            !gpu.gpu_supported_features_has(features, native),
            "{native}, which browsers do not have, is not offered"
        );
    }
}
