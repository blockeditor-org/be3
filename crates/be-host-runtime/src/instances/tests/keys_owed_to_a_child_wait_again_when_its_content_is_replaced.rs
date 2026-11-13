use super::*;

#[test]
fn keys_owed_to_a_child_wait_again_when_its_content_is_replaced() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    let pass = host::pass();
    assert!(instances.take_gift(INSTANCE, ChildId(1), notes_view(), false, (0, pass)));
    assert_eq!(
        notes_keys(
            &mut instances,
            progress((0, 0), true, false),
            true,
            vec![letter("T")],
            pass
        ),
        [],
        "the child is not ready for its keys"
    );
    let mut owed = |app| instances.settle_owed_keys(INSTANCE, &notes_view(), app, true, pass);
    assert_eq!(owed(progress((1, 1), true, false)), None);
    assert_eq!(
        owed(progress((2, 2), false, false)),
        None,
        "two frames went by but the content went away meanwhile, so the editor has nowhere to \
         put keys yet"
    );
    assert_eq!(
        owed(progress((3, 3), true, false)),
        None,
        "its frames count again"
    );
    assert_eq!(
        owed(progress((4, 4), true, true)),
        Some(beui::Modifiers::NONE),
        "its text field has the keyboard"
    );
}
