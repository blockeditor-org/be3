use super::*;

#[test]
fn a_new_ask_carries_the_keys_owed_to_the_first_child() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let pass = host::pass();
    let mut both = child(Rect::from_min_size(Pos2::ZERO, SIZE), false);
    let mut second = both.children[0].clone();
    second.child = ChildId(2);
    both.children.push(second);
    instances.set_children(both);
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    own_keys(
        &mut instances,
        progress((4, 4), true, false),
        true,
        vec![letter("T")],
        pass,
    );
    assert!(instances.take_gift(INSTANCE, ChildId(1), notes_view(), false, (0, pass)));
    assert_eq!(
        notes_keys(
            &mut instances,
            progress((0, 0), false, false),
            true,
            vec![letter("h")],
            pass
        ),
        [],
        "the first child is not ready, so its own key waits behind the held one"
    );
    tell(&mut instances, ShellMessage::FocusChild(ChildId(2)));
    let other: gift::GiftView = ("be3.text".to_owned(), EditorInstanceId(11), ViewId::MAIN);
    assert!(instances.take_gift(INSTANCE, ChildId(2), other.clone(), false, (0, pass)));
    let mut input = forwarded(Vec::new(), None, true);
    instances.owe_keys(
        INSTANCE,
        &other,
        progress((1, 1), true, true),
        &mut input,
        pass,
    );
    instances.owe_keys(
        INSTANCE,
        &other,
        progress((2, 2), true, true),
        &mut input,
        pass,
    );
    assert_eq!(
        input.events,
        [letter("T"), letter("h")],
        "the editor gave the keyboard on before the first child could take its keys, and they \
         follow it"
    );
}
