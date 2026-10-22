use super::*;

#[test]
fn a_narrow_window_shows_the_inspector_below_a_tab_bar() {
    let _root = crate::reactive::RootScope::new();
    let HelloColumn { document, .. } = hello_column();
    let mut harness = harness(document);

    harness.toggle_inspector();
    harness.frame(Vec::new());

    let panel = harness.inspector_panel_rect();
    assert_eq!(panel.width(), VIEWPORT.x);
    assert!(panel.top() >= harness.bar_option_rect(1).bottom());
}
