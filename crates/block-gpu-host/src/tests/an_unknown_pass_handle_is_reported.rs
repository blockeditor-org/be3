use super::*;

#[test]
fn an_unknown_pass_handle_is_reported() {
    let mut gpu = gpu();
    gpu.render_pass_draw(abi::RenderPass(404), 3, 1, 0, 0);
    let error = gpu.take_error().expect("the unknown pass should report");
    assert!(error.contains("render pass"), "{error}");
}
