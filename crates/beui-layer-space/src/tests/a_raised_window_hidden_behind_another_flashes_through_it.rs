use super::the_chrome_around_a_curved_panel::window;
use super::*;
use crate::FLASH_SECONDS;

#[test]
fn a_raised_window_hidden_behind_another_flashes_through_it() {
    let (mut space, now, context) = painted_space();
    let hidden = open_ahead(&mut space, now, 1.6);
    let press = centre() + vec2(-200.0, 150.0);
    send(
        &mut space,
        now,
        [
            Input::PointerMoved(press),
            button(press, true),
            button(press, false),
        ],
    );
    assert_eq!(space.focused(), Some(WindowId::MAIN));

    space.request(&WindowRequest::Raise(WindowId::MAIN));
    assert!(
        space.outlines().is_empty(),
        "the focused window needs no finding"
    );
    space.request(&WindowRequest::Raise(hidden));
    let outlines = space.outlines();
    assert_eq!(outlines.len(), 1, "a raised window behind another flashes");
    assert_eq!(outlines[0].strength, 1.0);
    assert_eq!(
        space.panel(hidden).unwrap().pose,
        panel(WindowId::MAIN, ahead(1.6)).pose,
        "and stays where it is"
    );
    let windows = [
        window(&context, WindowId::MAIN, PLATFORM, "Main"),
        window(&context, hidden, SMALL, "Hidden"),
    ];
    assert_scene(
        &mut space,
        now,
        &windows,
        "a_raised_window_hidden_behind_another_flashes_through_it",
    );

    send(&mut space, later(now, FLASH_SECONDS / 2.0), []);
    let strength = space.outlines()[0].strength;
    assert!(
        strength > 0.0 && strength < 1.0,
        "the flash fades: {strength}"
    );
    send(&mut space, later(now, FLASH_SECONDS * 1.1), []);
    assert!(space.outlines().is_empty() && !space.flashing());
}
