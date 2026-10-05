use super::*;

#[test]
fn a_drawn_window_is_listed_and_fitted_to_where_it_is_shown() {
    let mut harness = Harness::new();
    let (_window, id) = harness.open();

    let list = harness.app.windows().list().get_untracked();
    assert_eq!(list.len(), 1, "the window is listed once it has drawn");
    assert_eq!(list[0].id, id);
    assert_eq!(
        list[0].size,
        Vec2::new(300.0, 200.0),
        "it is listed at the size it drew"
    );
    let shown = harness.rect(&format!("wayland.window.{}", id.0));
    assert_eq!(shown.size(), SHOWN);
    assert_eq!(
        harness.client.received.size,
        Some((SHOWN.x as i32, SHOWN.y as i32)),
        "the client is told the size it is shown at"
    );
    assert!(harness.client.received.activated);
}
