use super::*;

#[test]
fn a_hosted_dock_dialog_closes_its_window_while_locked() {
    let _root = crate::reactive::RootScope::new();
    let Dock {
        mut harness,
        window,
        locked,
        ..
    } = hosted_dock(WIDE_VIEWPORT);
    locked.set(true);
    harness.frame(Vec::new());
    assert!(
        harness
            .take_window_requests()
            .contains(&WindowRequest::Close(window)),
        "the lock layer does not paint over it, so its window closes"
    );
}
