use super::web::*;
use super::*;

#[test]
fn a_surface_reports_how_many_presents_ago_its_texture_was_drawn() {
    let mut gpu = gpu();
    let device = device(&mut gpu, Vec::new()).unwrap();
    let context = canvas(&mut gpu, device, 0, (16, 8), Vec::new());
    let mut ages = Vec::new();
    for width in [16, 16, 16, 16, 32, 32, 32] {
        gpu.resize(context, width, 8);
        gpu.gpu_canvas_context_get_current_texture(context);
        ages.push(gpu.age(context));
        gpu.present(context);
    }
    assert_eq!(
        ages,
        vec![0, 0, 2, 2, 0, 0, 2],
        "a texture holds nothing until it is presented, then what it held two presents ago, and a new size starts over"
    );
    assert!(gpu.take_logged().is_empty());
}
