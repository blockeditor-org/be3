use super::*;

/// An app that starts asks for its content (`Resend`) though the host relayed it already: the
/// editor had none of it, and keys owed to its view wait for the content relayed again, even
/// when the host never saw it go between the two.
#[test]
fn keys_owed_to_a_child_wait_again_when_it_asks_for_its_content_again() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    let pass = host::pass();
    assert!(instances.take_gift(INSTANCE, ChildId(1), notes_view(), false, (0, pass)));
    let delivered = |frames, deliveries| gift::KeyProgress {
        frames,
        content: Some(deliveries),
        typing: false,
    };
    assert_eq!(
        notes_keys(
            &mut instances,
            delivered((0, 0), 1),
            true,
            vec![letter("T")],
            pass
        ),
        [],
        "the child is not ready for its keys"
    );
    let mut owed = |app| instances.settle_owed_keys(INSTANCE, &notes_view(), app, true, pass);
    assert_eq!(owed(delivered((1, 1), 1)), None);
    assert_eq!(
        owed(delivered((2, 2), 2)),
        None,
        "two frames went by, but the content was relayed again for the editor that asked"
    );
    assert_eq!(owed(delivered((3, 3), 2)), None, "its frames count again");
    assert_eq!(
        owed(delivered((4, 4), 2)),
        Some(beui::Modifiers::NONE),
        "two frames after the content it asked for"
    );
}
