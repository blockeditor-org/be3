use super::*;

#[test]
fn keys_typed_before_a_child_takes_the_keyboard_wait_for_it() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    let pass = host::pass();
    let settled = progress((4, 4), true, false);
    let give = |instances: &mut Instances| {
        tell(instances, ShellMessage::FocusChild(ChildId(1)));
    };
    give(&mut instances);
    assert_eq!(
        own_keys(&mut instances, settled, true, vec![letter("T")], pass),
        [],
        "the editor does not hear a key meant for its child"
    );
    assert_eq!(
        own_keys(&mut instances, settled, true, vec![letter("h")], pass),
        []
    );

    assert!(instances.take_gift(INSTANCE, ChildId(1), notes_view(), false, (7, pass)));
    assert_eq!(
        notes_keys(
            &mut instances,
            progress((7, 6), false, false),
            true,
            vec![beui::Forwarded::Focus(true), letter("a")],
            pass,
        ),
        [beui::Forwarded::Focus(true)],
        "while the child waits for its content, its own keys wait behind the held ones"
    );
    let loaded = progress((9, 9), true, false);
    assert_eq!(
        instances.settle_owed_keys(INSTANCE, &notes_view(), loaded, true, pass),
        None,
        "having the content and the focus is not enough: the field has yet to take the keyboard"
    );
    let drawn = progress((10, 10), true, true);
    assert_eq!(
        instances.settle_owed_keys(INSTANCE, &notes_view(), drawn, true, pass),
        Some(beui::Modifiers::NONE)
    );
    assert_eq!(
        notes_keys(&mut instances, drawn, true, Vec::new(), pass),
        [letter("T"), letter("h"), letter("a")],
        "in the order they were typed"
    );
    assert_eq!(
        notes_keys(&mut instances, drawn, true, vec![letter("n")], pass),
        [letter("n")]
    );

    give(&mut instances);
    assert!(instances.take_gift(INSTANCE, ChildId(1), notes_view(), true, (10, pass)));
    assert_eq!(
        notes_keys(&mut instances, drawn, true, vec![letter("o")], pass),
        [letter("o")],
        "a gift to a child that already has the keyboard, with nothing held, leaves its typing alone"
    );

    assert_eq!(
        own_keys(&mut instances, settled, true, vec![letter("x")], pass),
        [letter("x")],
        "with no gift pending the editor hears its keys"
    );

    give(&mut instances);
    assert_eq!(
        own_keys(&mut instances, settled, true, vec![letter("y")], pass),
        []
    );
    instances.end_gift(INSTANCE, gift::Keys::Return);
    assert_eq!(
        instances.settle_own_keys(INSTANCE, &own_view(), settled, true, pass),
        Some(beui::Modifiers::NONE)
    );
    assert_eq!(
        own_keys(&mut instances, settled, true, Vec::new(), pass),
        [letter("y")],
        "a gift that falls through gives the editor its keys back"
    );

    give(&mut instances);
    assert_eq!(
        own_keys(&mut instances, settled, true, vec![letter("z")], pass),
        []
    );
    assert_eq!(
        own_keys(
            &mut instances,
            settled,
            false,
            vec![beui::Forwarded::Focus(false)],
            pass
        ),
        [letter("z"), beui::Forwarded::Focus(false)],
        "taking the keyboard elsewhere ends the gift, and the editor hears its keys before it \
         lets go"
    );
    assert!(!instances.gift_pending(INSTANCE));

    give(&mut instances);
    assert_eq!(
        own_keys(&mut instances, settled, true, vec![letter("w")], pass),
        []
    );
    instances.end_gift(INSTANCE, gift::Keys::Return);
    assert_eq!(
        instances.settle_own_keys(INSTANCE, &own_view(), settled, false, pass),
        None,
        "keys given back to an editor that lost the keyboard are dropped"
    );
    assert_eq!(
        own_keys(&mut instances, settled, true, Vec::new(), pass),
        [],
        "and never replayed later"
    );

    give(&mut instances);
    assert_eq!(
        own_keys(&mut instances, settled, true, vec![letter("v")], pass),
        []
    );
    instances.set_children(ChildPlacements {
        children: Vec::new(),
        ..child(Rect::ZERO, false)
    });
    instances.settle_gifts(&[INSTANCE]);
    assert_eq!(
        instances.settle_own_keys(INSTANCE, &own_view(), settled, true, pass),
        Some(beui::Modifiers::NONE),
        "a gift that ends because the child went gives the keys back"
    );
    assert_eq!(
        own_keys(&mut instances, settled, true, Vec::new(), pass),
        [letter("v")]
    );
}
