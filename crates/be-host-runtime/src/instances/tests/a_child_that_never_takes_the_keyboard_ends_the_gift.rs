use super::*;

#[test]
fn a_child_that_never_takes_the_keyboard_ends_the_gift() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    let pass = host::pass();
    let settled = progress((4, 4), true, false);
    assert_eq!(
        own_keys(&mut instances, settled, true, vec![letter("T")], pass),
        [],
        "the key waits for the child"
    );
    for _ in 1..gift::GIFT_COMMITS {
        instances.settle_gifts(&[INSTANCE]);
    }
    assert!(
        instances.gift_pending(INSTANCE),
        "the editor may still be showing the child's view"
    );
    instances.settle_gifts(&[INSTANCE]);
    assert!(
        !instances.gift_pending(INSTANCE),
        "a child whose view never takes the keyboard, over the editor's own commits, ends it"
    );
    assert_eq!(
        instances.settle_own_keys(INSTANCE, &own_view(), settled, true, pass),
        Some(beui::Modifiers::NONE),
        "and the editor's view, which still has the keyboard, takes its keys back"
    );
    assert_eq!(
        own_keys(&mut instances, settled, true, Vec::new(), pass),
        [letter("T")]
    );
}
