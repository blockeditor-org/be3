use super::*;

#[test]
fn a_maximized_window_keeps_its_place_and_is_told_it_is_maximized() {
    let mut harness = Harness::new();
    let (window, id) = harness.open();
    let toplevel = window.toplevel.as_ref().unwrap();

    toplevel.set_maximized();
    harness.settle();
    assert!(harness.client.received.maximized);
    assert!(
        harness.client.received.configured.is_some(),
        "the request is answered with a configure"
    );
    assert_eq!(
        harness.client.received.size,
        Some((SHOWN.x as i32, SHOWN.y as i32)),
        "a maximized window fills the place it is shown in, which it already does"
    );
    assert_eq!(harness.window(id).size(), SHOWN);

    harness.acknowledge(&window);
    toplevel.unset_maximized();
    harness.settle();
    assert!(!harness.client.received.maximized);
    assert!(harness.client.received.configured.is_some());
}
