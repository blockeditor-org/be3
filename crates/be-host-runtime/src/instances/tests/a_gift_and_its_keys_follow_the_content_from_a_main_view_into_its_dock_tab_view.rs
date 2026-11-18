use super::*;

/// A joined editor's content moves from its main view, which took the gift, into its dock tab
/// view: the gift and the keys held for it go on to that view.
#[test]
fn a_gift_and_its_keys_follow_the_content_from_a_main_view_into_its_dock_tab_view() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let pass = host::pass();
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    own_keys(
        &mut instances,
        progress((4, 4), true, false),
        true,
        vec![letter("T")],
        pass,
    );
    assert!(instances.take_gift(INSTANCE, ChildId(1), notes_view(), false, (0, pass)));

    let dock: gift::GiftView = ("be3.text".to_owned(), EditorInstanceId(10), ViewId(1));
    instances.forget_keys(&notes_view(), Some(&dock), pass);
    let mut input = forwarded(Vec::new(), None, true);
    instances.owe_keys(
        INSTANCE,
        &dock,
        progress((1, 1), true, true),
        &mut input,
        pass,
    );
    instances.owe_keys(
        INSTANCE,
        &dock,
        progress((2, 2), true, true),
        &mut input,
        pass,
    );
    assert_eq!(
        input.events,
        [letter("T")],
        "the dock tab view the content moved to is owed the key held for the main view"
    );
}
