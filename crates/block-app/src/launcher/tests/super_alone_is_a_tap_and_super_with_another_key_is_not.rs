use super::*;

const KEY_T: u32 = 20;

#[test]
fn super_alone_is_a_tap_and_super_with_another_key_is_not() {
    let mut tap = SuperTap::default();
    assert!(!tap.feed(&[key(LEFT_SUPER, true)]));
    assert!(
        tap.feed(&[key(LEFT_SUPER, false)]),
        "pressing Super alone taps it"
    );

    assert!(
        tap.feed(&[key(RIGHT_SUPER, true), key(RIGHT_SUPER, false)]),
        "either Super key taps, even within one frame"
    );

    assert!(!tap.feed(&[key(LEFT_SUPER, true), key(KEY_T, true)]));
    assert!(
        !tap.feed(&[key(KEY_T, false), key(LEFT_SUPER, false)]),
        "Super held for a shortcut is not a tap"
    );

    assert!(!tap.feed(&[
        key(LEFT_SUPER, true),
        Event::PointerButton {
            pos: beui::Pos2::ZERO,
            button: beui::PointerButton::Primary,
            pressed: true,
            modifiers: beui::Modifiers::NONE,
        },
        key(LEFT_SUPER, false),
    ]));

    assert!(
        !tap.feed(&[key(LEFT_SUPER, false)]),
        "a release with no press is not a tap"
    );
}
