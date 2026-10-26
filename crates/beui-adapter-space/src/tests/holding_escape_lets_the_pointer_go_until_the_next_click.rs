use super::*;
use crate::hold::LET_GO_AFTER;

const ESCAPE: u32 = 1;

fn repeat() -> Input {
    Input::Key(Keystroke {
        key: Some(Key::Escape),
        pressed: true,
        repeat: true,
        evdev: Some(ESCAPE),
        ..Keystroke::default()
    })
}

#[test]
fn holding_escape_lets_the_pointer_go_until_the_next_click() {
    let (mut space, now) = space();
    assert!(space.grabbed(), "space holds the pointer from the start");
    send(&mut space, now, [Input::PointerMoved(centre())]);

    let pressed = send(&mut space, now, [key(Key::Escape, ESCAPE, true)]);
    assert_eq!(
        delivered(&pressed),
        [(WindowId::MAIN, key(Key::Escape, ESCAPE, true))],
        "the app sees the press at once"
    );

    let before = now + LET_GO_AFTER / 2;
    let repeated = send(&mut space, before, [repeat()]);
    assert_eq!(
        delivered(&repeated).len(),
        1,
        "repeats before the hold triggers still reach the app"
    );
    assert!(space.grabbed(), "a short hold keeps the grab");

    let held = now + LET_GO_AFTER;
    send(&mut space, held, []);
    assert!(!space.grabbed(), "holding Escape lets the pointer go");
    assert_eq!(
        delivered(&send(&mut space, later(held, 0.1), [repeat()])),
        [],
        "repeats after the hold triggers do not reach the app"
    );
    assert_eq!(
        delivered(&send(
            &mut space,
            later(held, 0.2),
            [key(Key::Escape, ESCAPE, false)]
        )),
        [(WindowId::MAIN, key(Key::Escape, ESCAPE, false))],
        "the release reaches the app so its key state stays right"
    );

    let at = centre() + vec2(40.0, 0.0);
    let moved = delivered(&send(&mut space, held, [Input::PointerMoved(at)]));
    assert_eq!(moved.len(), 1, "a free pointer still hovers the app");
    let clicked = send(&mut space, held, [button(at, true), button(at, false)]);
    assert_eq!(
        delivered(&clicked),
        [],
        "the click that grabs it again is swallowed"
    );
    assert!(space.grabbed());

    let got = delivered(&send(
        &mut space,
        held,
        [button(at, true), button(at, false)],
    ));
    assert_eq!(got.len(), 2, "the next click reaches the app: {got:?}");
    let output = app_frame(CursorIcon::Default);
    assert!(
        space
            .frame(&output, &view(held))
            .window
            .window
            .requests
            .pointer_locked,
        "while grabbed it asks for pointer lock"
    );

    send(&mut space, held, [key(Key::Escape, ESCAPE, true)]);
    assert_eq!(
        space.frame(&output, &view(held)).repaint_after,
        LET_GO_AFTER,
        "space asks for a frame when the hold is long enough, not before"
    );
}
