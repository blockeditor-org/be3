use super::*;

#[test]
fn the_focused_layer_that_asks_holds_the_pointer_lock_until_it_stops_asking() {
    let mut windows = Windows::new(ROOT);
    windows.drew(drawn());
    let screen_asks = |layer: &Layer| layer.id == SCREEN;
    windows.lock(screen_asks);
    assert_eq!(
        windows.locked(None),
        None,
        "the screen asks, but has no focus, which the lock needs"
    );

    windows.route(
        None,
        vec![button(150.0, true).into(), button(150.0, false).into()],
    );
    windows.lock(screen_asks);
    assert_eq!(
        windows.locked(None).map(|layer| layer.id),
        Some(SCREEN),
        "focused by a press, the screen holds the lock it asks for"
    );

    windows.lock(|_| false);
    assert_eq!(
        windows.locked(None),
        None,
        "once it stops asking, it lets go"
    );

    windows.lock(screen_asks);
    windows.route(
        None,
        vec![button(50.0, true).into(), button(50.0, false).into()],
    );
    windows.lock(screen_asks);
    assert_eq!(
        windows.locked(None),
        None,
        "a press on the root's own surface takes the focus, and the lock with it"
    );
}
