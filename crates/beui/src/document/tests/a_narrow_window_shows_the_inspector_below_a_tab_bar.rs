use super::*;

#[test]
fn a_narrow_window_shows_the_inspector_below_a_tab_bar() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.frame(Vec::new());

    let panel = harness.inspector_panel_rect();
    assert_eq!(panel.width(), VIEWPORT.x);
    assert!(panel.top() >= harness.bar_option_rect(1).bottom());
}
