use super::*;

/// A view given the keyboard that goes while its content shows nowhere else ends the gift, and
/// the keys held for it go with it.
#[test]
fn a_view_that_goes_with_nowhere_for_its_content_to_go_ends_its_gift() {
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

    instances.forget_keys(&notes_view(), None, pass);
    let dock: gift::GiftView = ("be3.text".to_owned(), EditorInstanceId(10), ViewId(1));
    let mut input = forwarded(Vec::new(), None, true);
    instances.owe_keys(
        INSTANCE,
        &dock,
        progress((2, 2), true, true),
        &mut input,
        pass,
    );
    assert_eq!(
        input.events,
        [],
        "the gift ended with its view: no other view is owed its keys"
    );
    assert_eq!(
        own_keys(
            &mut instances,
            progress((5, 5), true, false),
            false,
            Vec::new(),
            pass
        ),
        [],
        "the held key is dropped, not handed back to an editor view without the keyboard"
    );
}
