use super::*;

#[test]
fn a_new_instances_surfaces_never_share_a_generation_with_an_old_ones() {
    let generations = |gpu: &mut Gpu| {
        let surface = gpu.surface_new(0);
        gpu.surface_configure(surface, configuration(320, 200));
        let first = gpu.surface(0).expect("the surface has a texture").1;
        gpu.surface_acquire(surface);
        gpu.surface_present(surface);
        let second = gpu.surface(0).expect("the surface has a texture").1;
        [first, second]
    };
    let crashed = generations(&mut gpu());
    let restarted = generations(&mut gpu());
    assert!(
        restarted
            .iter()
            .all(|generation| !crashed.contains(generation)),
        "the restarted instance's surface {restarted:?} reuses the crashed one's {crashed:?}, so a presenter keeps showing the old textures"
    );
}
