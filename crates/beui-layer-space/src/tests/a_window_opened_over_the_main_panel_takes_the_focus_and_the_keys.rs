use super::*;

#[test]
fn a_window_opened_over_the_main_panel_takes_the_focus_and_the_keys() {
    let (mut space, now) = space();
    let lit = |space: &Space, window| space.chrome_opacity(window) == 1.0;
    let sky = pos2(5.0, 5.0);
    send(
        &mut space,
        now,
        [
            Input::PointerMoved(sky),
            button(sky, true),
            button(sky, false),
        ],
    );
    assert_eq!(space.focused(), None);
    assert_eq!(
        keys(&send(&mut space, now, [key(Key::A, 30, true)])),
        0,
        "keys reach no window while none is focused"
    );
    send(&mut space, now, [key(Key::A, 30, false)]);
    let on_main = screen(&space, main(&space).world(pos2(60.0, 60.0)));
    send(
        &mut space,
        now,
        [
            Input::PointerMoved(on_main),
            button(on_main, true),
            button(on_main, false),
        ],
    );
    assert!(lit(&space, WindowId::MAIN));

    let over = WindowPlacement::Over {
        window: WindowId::MAIN,
        at: pos2(64.0, 48.0),
    };
    let (window, sent) = open(&mut space, now, over.clone());
    assert_eq!(
        focus_reports(&sent),
        [(WindowId::MAIN, false), (window, true)],
        "opening a window moves the focus to it, as a window manager does"
    );
    assert_eq!(space.focused(), Some(window));
    assert!(
        lit(&space, window) && !lit(&space, WindowId::MAIN),
        "its chrome lights up and the main panel's dims"
    );
    assert_eq!(
        keys(&send(&mut space, now, [key(Key::A, 30, true)])),
        1,
        "keys reach the app for the window just opened"
    );
    send(&mut space, now, [key(Key::A, 30, false)]);

    space.request(&WindowRequest::Open {
        window,
        placement: over.clone(),
        size: SMALL,
        role: WindowRole::Normal,
    });
    let (second, sent) = open(&mut space, now, over);
    assert_eq!(
        focus_reports(&sent),
        [(window, false), (second, true)],
        "asking again for an open window changes nothing, and the last window opened has the focus"
    );
}
