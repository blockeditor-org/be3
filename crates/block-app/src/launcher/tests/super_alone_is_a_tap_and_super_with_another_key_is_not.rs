use super::*;

use beui::Modifiers;

#[test]
fn super_alone_is_a_tap_and_super_with_another_key_is_not() {
    let mut tap = SuperTap::default();
    assert!(!tap.feed(&[press(Key::Logo), held(Modifiers::LOGO)]));
    assert!(
        tap.feed(&[release(Key::Logo), held(Modifiers::NONE)]),
        "pressing Super alone taps it"
    );

    assert!(
        tap.feed(&[press(Key::Logo), release(Key::Logo)]),
        "a tap within one frame counts"
    );

    assert!(
        tap.feed(&[
            held(Modifiers::LOGO),
            held(Modifiers::NONE),
            press(Key::Logo),
            held(Modifiers::LOGO),
            release(Key::Logo),
            held(Modifiers::NONE),
        ]),
        "a modifier state that comes and goes ahead of the press is one tap, not two"
    );

    assert!(!tap.feed(&[press(Key::Logo), key(Key::T, true, Modifiers::LOGO)]));
    assert!(
        !tap.feed(&[key(Key::T, false, Modifiers::LOGO), release(Key::Logo)]),
        "Super held for a shortcut is not a tap"
    );

    assert!(!tap.feed(&[
        press(Key::Logo),
        key(Key::Shift, true, Modifiers::LOGO),
        key(Key::Shift, false, Modifiers::LOGO),
        release(Key::Logo),
    ]));
    assert!(
        !tap.feed(&[
            press(Key::Ctrl),
            key(Key::Logo, true, Modifiers::CTRL),
            key(Key::Logo, false, Modifiers::CTRL),
        ]),
        "Super added to a held Ctrl is not a tap"
    );
    tap.feed(&[release(Key::Ctrl)]);

    assert!(!tap.feed(&[
        press(Key::Logo),
        Event::PointerButton {
            pos: beui::Pos2::ZERO,
            button: beui::PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::LOGO,
        },
        release(Key::Logo),
    ]));

    assert!(
        !tap.feed(&[release(Key::Logo)]),
        "a release with no press is not a tap"
    );
}
