use super::*;

#[test]
fn asking_again_for_the_same_child_keeps_the_count_going() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    let up = host::now();
    instances.start_gift_count(INSTANCE, ChildId(1), up);
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    assert_eq!(
        instances.expire_gifts(up + gift::GIFT_WAIT / 2),
        Some(gift::GIFT_WAIT / 2),
        "the child's view, already up, does not say so again, so the count started then goes on"
    );
    assert_eq!(instances.expire_gifts(up + gift::GIFT_WAIT), None);
    assert!(
        !instances.gift_pending(INSTANCE),
        "a gift asked twice still ends when its child never takes it"
    );
}
