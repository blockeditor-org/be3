use super::*;

#[test]
fn closing_a_window_frees_its_texture() {
    let (mut harness, _device, _queue) = Harness::with_gpu();
    let mut textures = Vec::new();
    for _ in 0..4 {
        let (window, id) = harness.open();
        let surface = harness.compositor().server().state.surface(id).unwrap();
        let current = harness
            .compositor()
            .textures
            .borrow()
            .get(&surface)
            .expect("the window has a texture");
        textures.push(Rc::downgrade(&current.texture));
        drop(current);
        drop(surface);
        window.toplevel.as_ref().unwrap().destroy();
        window.xdg_surface.destroy();
        window.surface.destroy();
        harness.settle();
    }

    let live = textures
        .iter()
        .filter(|texture| texture.upgrade().is_some())
        .count();
    assert_eq!(live, 0, "every closed window's texture is freed");
}
