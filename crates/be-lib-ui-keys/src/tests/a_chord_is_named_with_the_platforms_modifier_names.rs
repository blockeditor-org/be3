use super::*;

#[test]
fn a_chord_is_named_with_the_platforms_modifier_names() {
    let quick_open = KeyChord::new(Key::P, Modifiers::CTRL);
    let palette = KeyChord::new(
        Key::P,
        Modifiers {
            shift: true,
            ..Modifiers::CTRL
        },
    );
    let global = KeyChord::new(
        Key::L,
        Modifiers {
            alt: true,
            ..Modifiers::LOGO
        },
    );

    assert_eq!(quick_open.label_as(false), "Ctrl+P");
    assert_eq!(palette.label_as(false), "Ctrl+Shift+P");
    assert_eq!(global.label_as(false), "Super+Alt+L");
    assert_eq!(KeyChord::tap(Key::Alt).label_as(false), "Alt");

    assert_eq!(
        quick_open.label_as(true),
        "Cmd+P",
        "Cmd is held as ctrl on Apple's systems, so ctrl is named Cmd there"
    );
    assert_eq!(palette.label_as(true), "Cmd+Shift+P");
    assert_eq!(
        KeyChord::tap(Key::Alt).label_as(true),
        "Option",
        "and alt is Option"
    );

    assert_eq!(
        quick_open.label(),
        quick_open.label_as(cfg!(target_vendor = "apple")),
        "a build names chords as its target does until a platform says otherwise"
    );
}
