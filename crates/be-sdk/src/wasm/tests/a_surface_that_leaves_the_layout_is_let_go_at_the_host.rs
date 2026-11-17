use super::*;

/// A surface the app's layout stops naming is released through `Surfaces::place`, which drops it
/// at the host as the app's own drop would: the host then holds no texture for it, so a sandbox
/// instance lets go of what it presented there too (plans/host_as_platform_adapter.md, 4e2).
#[test]
fn a_surface_that_leaves_the_layout_is_let_go_at_the_host() {
    let mut rig = Rig::new(Some(space()));
    rig.open(vec![layer(1, 0, Rect::from_min_size(pos2(220.0, 130.0), vec2(60.0, 40.0)))]);
    rig.draw();
    let [(_, surface)] = rig.anchored_screens()[..] else {
        panic!("one anchored view is open");
    };

    rig.open(Vec::new());
    rig.draw();
    assert!(rig.anchored_screens().is_empty(), "its screen left the layout");
    assert!(
        be_wasm_gpu_guest::acquire_surface_texture(surface).is_err(),
        "the host no longer holds the surface, though the app never dropped it itself"
    );
    be_wasm_gpu_guest::release_surface(surface);
}
