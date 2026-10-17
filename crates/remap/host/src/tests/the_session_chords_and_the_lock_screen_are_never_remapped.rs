use super::*;

#[test]
fn the_session_chords_and_the_lock_screen_are_never_remapped() {
    let mut remapper = remapper(
        r#"
        [[group]]
        rules = [
            { on = "Backspace", send = "X" },
            { on = "LeftAlt", send = "Z" },
            { on = "L", send = "Y" },
            { on = "MouseLeft", send = "J" },
        ]
        "#,
    );

    assert_eq!(
        remapper.input(press("LeftCtrl"), NOW),
        vec![press("LeftCtrl")]
    );
    assert_eq!(remapper.input(press("LeftAlt"), NOW), vec![press("Z")]);
    assert_eq!(
        remapper.input(press("Backspace"), NOW),
        vec![press("LeftAlt"), press("Backspace")],
        "Ctrl+Alt+Backspace reaches the session with the Alt it needs"
    );
    assert_eq!(
        remapper.input(release("Backspace"), NOW),
        vec![release("Backspace")]
    );

    assert_eq!(
        remapper.input(press("LeftSuper"), NOW),
        vec![press("LeftSuper")]
    );
    assert_eq!(
        remapper.input(press("L"), NOW),
        vec![press("L")],
        "Super+L locks"
    );

    assert_eq!(remapper.input(press("MouseLeft"), NOW), vec![press("J")]);
    assert_eq!(
        remapper.set_locked(true),
        vec![release("J"), release("Z")],
        "locking lets go of what the remaps held"
    );
    assert_eq!(remapper.input(release("MouseLeft"), NOW), vec![]);
    assert_eq!(
        remapper.input(press("MouseLeft"), NOW),
        vec![press("MouseLeft")]
    );
    assert_eq!(
        remapper.input(release("MouseLeft"), NOW),
        vec![release("MouseLeft")]
    );
}
