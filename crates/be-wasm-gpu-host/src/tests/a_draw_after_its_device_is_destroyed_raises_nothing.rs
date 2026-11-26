use super::web::*;
use super::*;

#[test]
fn a_draw_after_its_device_is_destroyed_raises_nothing() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let pass = drawing_pass(&mut gpu, device);
    gpu.gpu_device_destroy(device);
    gpu.gpu_render_pass_encoder_draw(pass, 3, None, None, None);
    gpu.gpu_render_pass_encoder_end(pass);
    gpu.gpu_render_pass_encoder_draw(pass, 3, None, None, None);
    assert!(gpu.take_logged().is_empty(), "{:?}", gpu.take_logged());
}
