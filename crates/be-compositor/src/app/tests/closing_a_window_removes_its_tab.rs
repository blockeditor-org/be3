use super::*;

#[test]
fn closing_a_window_removes_its_tab() {
    let mut harness = Harness::new();
    let (window, id) = harness.open();
    window.toplevel.as_ref().unwrap().destroy();
    window.xdg_surface.destroy();
    harness.settle();

    assert!(
        !harness
            .app
            .clients()
            .dock()
            .get_untracked()
            .contains(tab_of(id))
    );
    assert!(harness.app.clients().order().get_untracked().is_empty());
    assert!(
        harness
            .output
            .as_ref()
            .unwrap()
            .test_id_rect("compositor.window.1")
            .is_none(),
        "the window is gone from the launcher's list"
    );
}
