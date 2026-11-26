use super::web::*;
use super::*;

#[test]
fn a_new_instances_surfaces_never_share_a_generation_with_an_old_ones() {
    let generations = |gpu: &mut Gpu| {
        let device = device(gpu, Vec::new()).unwrap();
        let context = canvas(gpu, device, 0, (320, 200), Vec::new());
        let first = Gpu::surface(gpu, 0).expect("the surface has a texture").1;
        gpu.gpu_canvas_context_get_current_texture(context);
        gpu.present(context);
        let second = Gpu::surface(gpu, 0).expect("the surface has a texture").1;
        [first, second]
    };
    let crashed = generations(&mut gpu());
    let restarted = generations(&mut gpu());
    assert!(
        restarted.iter().all(|generation| !crashed.contains(generation)),
        "the restarted instance's surface {restarted:?} reuses the crashed one's {crashed:?}, so a presenter keeps showing the old textures"
    );
}
