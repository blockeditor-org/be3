use super::*;
use crate::hold::LET_GO_AFTER;

const ESCAPE: u32 = 1;

#[test]
fn tapping_escape_reaches_the_app_and_keeps_the_grab() {
    let (mut space, now) = space();
    send(&mut space, now, [Input::PointerMoved(centre())]);

    let tapped = send(
        &mut space,
        now,
        [key(Key::Escape, ESCAPE, true), key(Key::Escape, ESCAPE, false)],
    );
    assert_eq!(
        delivered(&tapped),
        [
            (WindowId::MAIN, key(Key::Escape, ESCAPE, true)),
            (WindowId::MAIN, key(Key::Escape, ESCAPE, false)),
        ],
        "a tap reaches the app in the same frame"
    );
    send(&mut space, now + LET_GO_AFTER * 2, []);
    assert!(space.grabbed(), "a tap keeps the grab");

    send(&mut space, now, steer(true));
    let escaped = send(
        &mut space,
        now,
        [key(Key::Escape, ESCAPE, true), key(Key::Escape, ESCAPE, false)],
    );
    send(&mut space, now + LET_GO_AFTER * 2, []);
    send(&mut space, now, steer(false));
    assert_eq!(delivered(&escaped), [], "keys while steering stay with space");
    assert!(space.grabbed(), "Ctrl+Shift+Escape no longer lets go");
}
