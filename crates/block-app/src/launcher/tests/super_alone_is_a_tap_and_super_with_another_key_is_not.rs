use super::*;

#[test]
fn super_alone_is_a_tap_and_super_with_another_key_is_not() {
    let mut tap = SuperTap::default();
    assert!(!tap.feed(&[held(Modifiers::LOGO)]));
    assert!(
        tap.feed(&[held(Modifiers::NONE)]),
        "pressing Super alone taps it"
    );

    assert!(
        tap.feed(&[held(Modifiers::LOGO), held(Modifiers::NONE)]),
        "a tap within one frame counts"
    );

    assert!(!tap.feed(&[held(Modifiers::LOGO), key(beui::Key::T, true)]));
    assert!(
        !tap.feed(&[key(beui::Key::T, false), held(Modifiers::NONE)]),
        "Super held for a shortcut is not a tap"
    );

    assert!(!tap.feed(&[
        held(Modifiers::LOGO),
        held(Modifiers {
            shift: true,
            ..Modifiers::LOGO
        }),
        held(Modifiers::LOGO),
        held(Modifiers::NONE),
    ]));
    assert!(
        !tap.feed(&[held(Modifiers::CTRL), held(Modifiers::LOGO)]),
        "Super added to a held Ctrl is not armed"
    );
    tap.feed(&[held(Modifiers::NONE)]);

    assert!(!tap.feed(&[
        held(Modifiers::LOGO),
        Event::PointerButton {
            pos: beui::Pos2::ZERO,
            button: beui::PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::LOGO,
        },
        held(Modifiers::NONE),
    ]));

    assert!(
        !tap.feed(&[held(Modifiers::NONE)]),
        "a release with no press is not a tap"
    );
}
