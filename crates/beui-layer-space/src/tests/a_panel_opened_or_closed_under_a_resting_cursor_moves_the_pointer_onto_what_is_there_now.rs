use super::*;

#[test]
fn a_panel_opened_or_closed_under_a_resting_cursor_moves_the_pointer_onto_what_is_there_now() {
    let (mut space, now) = space();
    let rest = pos2(200.0, 170.0);
    let at = screen(&space, main(&space).world(rest));
    send(&mut space, now, [Input::PointerMoved(at)]);

    let popup = WindowId::next();
    space.request(&WindowRequest::Open {
        window: popup,
        placement: WindowPlacement::Over {
            window: WindowId::MAIN,
            at: pos2(150.0, 120.0),
        },
        size: SMALL,
        role: WindowRole::Popup,
        level: beui_types::WindowLevel::Normal,
    });
    let got = delivered(&send(&mut space, now, []));
    let [(window, moved)] = got.as_slice() else {
        panic!("one move reaches the popup that opened under the cursor, not {got:?}");
    };
    assert_eq!(*window, popup);
    let hit = space.pick(space.cursor_ray().unwrap()).unwrap();
    assert_eq!(hit.window, popup);
    assert_eq!(
        pos_of(moved),
        hit.pos,
        "at the point where the cursor's ray meets the popup"
    );
    assert_eq!(
        delivered(&send(&mut space, now, [])),
        [],
        "and nothing more while nothing changes"
    );

    space.close(popup);
    let got = delivered(&send(&mut space, now, []));
    let [(window, moved)] = got.as_slice() else {
        panic!("one move reaches the main window once the popup closes, not {got:?}");
    };
    assert_eq!(*window, WindowId::MAIN);
    assert!(
        (pos_of(moved) - rest).length() < 2.0,
        "at the point the cursor rests on"
    );
}
