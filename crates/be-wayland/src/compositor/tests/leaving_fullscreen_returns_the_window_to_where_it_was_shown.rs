use super::*;

#[test]
fn leaving_fullscreen_returns_the_window_to_where_it_was_shown() {
    let mut harness = Harness::new();
    let (window, id) = harness.open();
    let shown = harness.window(id);
    let toplevel = window.toplevel.as_ref().unwrap();

    toplevel.set_fullscreen(None);
    harness.acknowledge(&window);
    assert!(harness.client.received.fullscreen);

    toplevel.unset_fullscreen();
    harness.acknowledge(&window);

    assert_eq!(harness.window(id), shown, "the window is back in its place");
    assert_eq!(
        harness.client.received.size,
        Some((SHOWN.x as i32, SHOWN.y as i32)),
        "the client is told the size of its place again"
    );
    assert!(!harness.client.received.fullscreen);
    assert_eq!(
        harness.app.windows().list().get_untracked()[0].fullscreen,
        None,
        "the ui is told the window no longer wants the screen"
    );
}
