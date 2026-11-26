use super::web::*;
use super::*;

#[test]
fn an_unknown_pass_handle_is_reported() {
    let mut gpu = gpu();
    gpu.gpu_render_pass_encoder_draw(GpuRenderPassEncoder(404), 3, None, None, None);
    let logged = gpu.take_logged();
    assert!(
        logged.iter().any(|error| error.contains("render pass")),
        "the unknown pass is logged: {logged:?}"
    );
}
