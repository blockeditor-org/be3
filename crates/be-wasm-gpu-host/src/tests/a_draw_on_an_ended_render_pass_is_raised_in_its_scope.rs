use super::web::*;
use super::*;

#[test]
fn a_draw_on_an_ended_render_pass_is_raised_in_its_scope() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let pass = drawing_pass(&mut gpu, device);
    gpu.gpu_render_pass_encoder_end(pass);
    let refused = scoped(&mut gpu, device, |gpu| {
        gpu.gpu_render_pass_encoder_draw(pass, 3, None, None, None);
    })
    .expect("a draw on an ended pass is refused");
    assert!(refused.contains("render pass"), "{refused}");
    assert!(
        gpu.take_logged().is_empty(),
        "the scope heard it, not the log"
    );
}
