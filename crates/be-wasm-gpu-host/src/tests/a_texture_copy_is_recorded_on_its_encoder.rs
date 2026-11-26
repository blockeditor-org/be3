use super::web::*;
use super::*;

#[test]
fn a_texture_copy_is_recorded_on_its_encoder() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let queue = gpu.gpu_device_queue(device);
    let source = texture(&mut gpu, device, GpuTextureUsage::COPY_SRC);
    let destination = texture(&mut gpu, device, GpuTextureUsage::COPY_DST);
    let whole = |texture: GpuTexture| GpuTexelCopyTextureInfo {
        texture,
        mip_level: None,
        origin: None,
        aspect: None,
    };
    let size = || GpuExtent3D { width: 8, height: Some(8), depth_or_array_layers: None };
    let encoder = gpu.gpu_device_create_command_encoder(device, None);
    let recorded = scoped(&mut gpu, device, |gpu| {
        gpu.gpu_command_encoder_copy_texture_to_texture(encoder, whole(GpuTexture(source.0)), whole(GpuTexture(destination.0)), size());
        let commands = gpu.gpu_command_encoder_finish(encoder, None);
        gpu.gpu_queue_submit(queue, &[commands]);
    });
    assert_eq!(recorded, None);
    let error = scoped(&mut gpu, device, |gpu| {
        gpu.gpu_command_encoder_copy_texture_to_texture(encoder, whole(GpuTexture(source.0)), whole(GpuTexture(destination.0)), size());
    })
    .expect("a copy on a finished encoder is refused");
    assert!(error.contains("command encoder"), "{error}");
}
