use super::*;

#[test]
fn the_ui_can_make_a_window_fullscreen_and_take_it_back() {
    let mut harness = Harness::new();
    let (window, id) = harness.open();

    harness.app.windows().request_fullscreen(id, true);
    harness.settle();
    assert!(
        harness.client.received.fullscreen,
        "the client is told it is fullscreen"
    );
    assert_eq!(harness.window(id).size(), SCREEN);

    harness.acknowledge(&window);
    harness.app.windows().request_fullscreen(id, false);
    harness.settle();
    assert!(!harness.client.received.fullscreen);
    assert_eq!(harness.window(id).size(), SHOWN);
}
