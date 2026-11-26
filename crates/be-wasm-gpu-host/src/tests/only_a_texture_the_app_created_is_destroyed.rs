use super::web::*;
use super::*;

#[test]
fn only_a_texture_the_app_created_is_destroyed() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let context = canvas(&mut gpu, device, 0, (4, 4), Vec::new());
    let shown = gpu.gpu_canvas_context_get_current_texture(context);
    gpu.gpu_texture_destroy(GpuTexture(shown.0));
    let viewed = scoped(&mut gpu, device, |gpu| {
        gpu.gpu_texture_create_view(GpuTexture(shown.0), None);
    });
    assert_eq!(viewed, None, "the canvas's texture was destroyed by the app");

    let made = texture(&mut gpu, device, GpuTextureUsage::TEXTURE_BINDING);
    gpu.gpu_texture_destroy(GpuTexture(made.0));
    let viewed = scoped(&mut gpu, device, |gpu| {
        gpu.gpu_texture_create_view(GpuTexture(made.0), None);
    });
    assert!(viewed.is_some(), "a destroyed texture made a view");
}
