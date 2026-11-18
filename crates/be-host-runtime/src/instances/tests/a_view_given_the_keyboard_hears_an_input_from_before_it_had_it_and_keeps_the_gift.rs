use super::*;

/// The child's view hears an input sent before it had the keyboard (a pointer resting over the
/// view as it was laid out) in the pass it was given the keyboard: it has until the next pass to
/// be seen with it, so the gift and the keys held for it stand.
#[test]
fn a_view_given_the_keyboard_hears_an_input_from_before_it_had_it_and_keeps_the_gift() {
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
    assert!(instances.take_gift(INSTANCE, ChildId(1), notes_view(), false, (0, pass)));
    let mut input = forwarded(Vec::new(), Some(pos2(20.0, 20.0)), false);
    instances.owe_keys(
        INSTANCE,
        &notes_view(),
        progress((1, 1), true, true),
        &mut input,
        pass,
    );
    notes_keys(
        &mut instances,
        progress((1, 1), true, true),
        true,
        Vec::new(),
        pass,
    );
    assert_eq!(
        notes_keys(
            &mut instances,
            progress((2, 2), true, true),
            true,
            Vec::new(),
            pass + 1
        ),
        [letter("T")],
        "once seen with the keyboard, the view is owed the key held for it"
    );
}
