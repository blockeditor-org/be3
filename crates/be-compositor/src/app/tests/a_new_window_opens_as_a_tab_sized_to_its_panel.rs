use super::*;

#[test]
fn a_new_window_opens_as_a_tab_sized_to_its_panel() {
    let mut harness = Harness::new();
    let (_window, id) = harness.open();

    assert!(
        harness
            .app
            .clients()
            .dock()
            .get_untracked()
            .contains(tab_of(id)),
        "the window has a tab of its own"
    );
    harness.rect("compositor.window.1");
    let panel = harness.rect("compositor.client.1");
    assert_eq!(
        harness.client.received.size,
        Some((panel.width().round() as i32, panel.height().round() as i32)),
        "the window is asked to fill its panel"
    );
    assert!(
        harness.client.received.activated,
        "a window that just opened has the focus"
    );
}
