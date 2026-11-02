use super::*;
use crate::test_client::PointerSeen;
use beui::vec2;

#[test]
fn a_locked_pointer_holds_still_and_moves_by_the_platforms_raw_motion_until_the_host_loses_focus() {
    let mut harness = Harness::new();
    let pointer = harness.client.pointer();
    let _relative = harness.client.relative_pointer(&pointer);
    let (window, id) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));
    let locked = harness.client.lock_pointer(&window.surface, &pointer);
    harness.settle();
    assert_eq!(
        harness.client.received.protocols.pointer,
        [PointerSeen::Locked],
        "a window with the keyboard and the pointer gets its lock"
    );
    assert!(
        pointer_locked(&harness),
        "the host holds the platform's pointer for the window"
    );

    let at = harness.client.received.pointer_at;
    harness.client.received.protocols.pointer.clear();
    let center = harness.window(id).center();
    harness.frame(vec![
        Input::PointerMotion(vec2(5.0, -3.0)),
        Input::PointerMoved(center + vec2(40.0, 20.0)),
    ]);
    harness.settle();
    assert_eq!(
        harness.client.received.protocols.pointer,
        [PointerSeen::Relative(5.0, -3.0, 5.0, -3.0)],
        "the platform's raw motion reaches the window as relative motion"
    );
    assert_eq!(
        harness.client.received.pointer_at, at,
        "the locked pointer does not move"
    );

    locked.set_cursor_position_hint(20.0, 30.0);
    window.surface.commit();
    harness.settle();
    harness.client.received.protocols.pointer.clear();
    harness.frame(vec![Input::Focus(false)]);
    harness.settle();
    assert_eq!(
        harness.client.received.protocols.pointer,
        [PointerSeen::Unlocked],
        "the lock ends once the host loses the keyboard"
    );
    assert!(!pointer_locked(&harness));
    assert_eq!(
        harness.client.received.pointer_at,
        Some((20.0, 30.0)),
        "the pointer is left where the window drew it"
    );
}

fn pointer_locked(harness: &Harness) -> bool {
    harness
        .output
        .as_ref()
        .expect("a frame ran")
        .main()
        .window
        .requests
        .pointer_locked
}
