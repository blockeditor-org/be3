use super::*;

#[test]
fn a_closed_window_leaves_the_list() {
    let mut harness = Harness::new();
    let (window, id) = harness.open();
    window.toplevel.as_ref().unwrap().destroy();
    window.xdg_surface.destroy();
    harness.settle();

    assert!(harness.app.windows().list().get_untracked().is_empty());
    assert!(harness.app.windows().signals(id).is_none());
    assert!(
        harness
            .output
            .as_ref()
            .unwrap()
            .test_id_rect(&format!("wayland.window.{}", id.0))
            .is_none(),
        "the window is no longer shown"
    );
}
