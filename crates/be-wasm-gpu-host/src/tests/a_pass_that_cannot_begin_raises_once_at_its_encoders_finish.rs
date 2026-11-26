use super::web::*;
use super::*;

#[test]
fn a_pass_that_cannot_begin_raises_once_at_its_encoders_finish() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let encoder = gpu.gpu_device_create_command_encoder(device, None);
    let mut pass = None;
    let begun = scoped(&mut gpu, device, |gpu| {
        let begun =
            gpu.gpu_command_encoder_begin_render_pass(encoder, pass_into(GpuTextureView(404)));
        gpu.gpu_render_pass_encoder_draw(begun, 3, None, None, None);
        gpu.gpu_render_pass_encoder_end(begun);
        pass = Some(begun);
    });
    assert_eq!(begun, None, "an invalid pass's calls raise nothing");
    let finished = scoped(&mut gpu, device, |gpu| {
        gpu.gpu_command_encoder_finish(encoder, None);
    })
    .expect("its encoder's finish raises why");
    assert!(finished.contains("texture view"), "{finished}");
    assert!(gpu.take_logged().is_empty());
}
