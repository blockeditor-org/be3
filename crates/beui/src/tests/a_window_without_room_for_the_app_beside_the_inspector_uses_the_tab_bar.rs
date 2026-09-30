use super::*;

#[test]
fn a_window_without_room_for_the_app_beside_the_inspector_uses_the_tab_bar() {
    let HelloColumn { document, .. } = hello_column();
    let viewport = Vec2::new(700.0, 600.0);
    let mut harness = Harness::sized(document, viewport);

    harness.toggle_inspector();
    harness.frame(Vec::new());

    assert_eq!(harness.inspector_panel_rect().width(), viewport.x);
}
