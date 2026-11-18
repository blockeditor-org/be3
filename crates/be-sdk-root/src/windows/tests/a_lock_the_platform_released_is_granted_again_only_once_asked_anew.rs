use super::*;

#[test]
fn a_lock_the_platform_released_is_granted_again_only_once_asked_anew() {
    let mut windows = Windows::new(ROOT);
    windows.drew(drawn());
    let screen_asks = |layer: &Layer| layer.id == SCREEN;
    windows.route(None, vec![button(150.0, true).into(), button(150.0, false).into()]);
    windows.lock(screen_asks);
    assert_eq!(windows.locked(None).map(|layer| layer.id), Some(SCREEN));

    let released = windows.route(None, vec![Input::PointerUnlocked.into()]);
    assert_eq!(
        released.into_iter().map(|delivery| delivery.input).collect::<Vec<_>>(),
        vec![Input::PointerUnlocked],
        "the screen hears the platform released its lock"
    );
    windows.lock(screen_asks);
    assert_eq!(
        windows.locked(None),
        None,
        "the screen still asks, but only as it did before the user let go, so it is not locked \
         again"
    );

    windows.lock(|_| false);
    windows.lock(screen_asks);
    assert_eq!(
        windows.locked(None).map(|layer| layer.id),
        Some(SCREEN),
        "once it stopped asking and asks anew, it gets the lock"
    );
}
