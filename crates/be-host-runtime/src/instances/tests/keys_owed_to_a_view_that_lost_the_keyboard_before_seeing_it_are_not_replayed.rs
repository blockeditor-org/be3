use super::*;

#[test]
fn keys_owed_to_a_view_that_lost_the_keyboard_before_seeing_it_are_not_replayed() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    let pass = host::pass();
    let settled = progress((4, 4), true, true);
    own_keys(&mut instances, settled, true, vec![letter("T")], pass);
    assert!(instances.take_gift(INSTANCE, ChildId(1), notes_view(), false, (4, pass)));
    assert_eq!(
        notes_keys(
            &mut instances,
            progress((9, 9), true, true),
            true,
            vec![beui::Forwarded::Focus(true), letter("x")],
            pass + 5,
        ),
        [beui::Forwarded::Focus(true), letter("x")],
        "the keyboard went elsewhere before the notes' view saw it, so when it comes back later \
         the notes hear only what is typed now"
    );
    assert_eq!(
        instances.settle_own_keys(INSTANCE, &own_view(), settled, true, pass + 5),
        None,
        "nor does the editor's view get them back so long after"
    );
    assert!(instances.entries[&INSTANCE].gift.is_none());
}
