use super::*;

#[test]
fn a_stale_input_method_does_not_release_owed_keys() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    let pass = host::pass();
    own_keys(
        &mut instances,
        progress((4, 4), true, false),
        true,
        vec![letter("T")],
        pass,
    );
    assert!(instances.take_gift(INSTANCE, ChildId(1), notes_view(), false, (5, pass)));
    let stale = progress((5, 5), false, true);
    assert_eq!(
        notes_keys(
            &mut instances,
            stale,
            true,
            vec![beui::Forwarded::Focus(true)],
            pass
        ),
        [beui::Forwarded::Focus(true)],
        "the field that asked for an input method did so in a frame drawn before the notes had \
         the focus"
    );
    assert_eq!(
        instances.settle_owed_keys(INSTANCE, &notes_view(), stale, true, pass),
        None
    );
    let answered = progress((6, 6), false, true);
    assert_eq!(
        instances.settle_owed_keys(INSTANCE, &notes_view(), answered, true, pass + 1),
        Some(beui::Modifiers::NONE),
        "a frame answered after the focus still asks for it"
    );
    assert_eq!(
        notes_keys(&mut instances, answered, true, Vec::new(), pass + 1),
        [letter("T")]
    );
}
