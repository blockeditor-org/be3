use super::*;

#[test]
fn a_new_window_floats_at_the_size_it_drew() {
    let mut harness = Harness::new();
    let (_first, first) = harness.open();
    let (_second, second) = harness.open();

    let state = harness.app.clients().dock().get_untracked();
    let surface = |id: WindowId| {
        state
            .find(tab_of(id))
            .expect("the window has a tab")
            .surface
    };
    assert_ne!(surface(first), state.main(), "a new window floats");
    assert_ne!(
        surface(first),
        surface(second),
        "each window gets a floating window of its own"
    );
    assert_ne!(
        state.window_rect(surface(first)).unwrap().min,
        state.window_rect(surface(second)).unwrap().min,
        "windows cascade rather than stacking exactly"
    );

    let panel = harness.rect(&format!("compositor.client.{}", second.0));
    assert_eq!(
        (panel.width().round(), panel.height().round()),
        (300.0, 200.0),
        "the panel is fitted to what the client drew"
    );
    assert_eq!(harness.client.received.size, Some((300, 200)));
    assert!(harness.client.received.activated);
}
