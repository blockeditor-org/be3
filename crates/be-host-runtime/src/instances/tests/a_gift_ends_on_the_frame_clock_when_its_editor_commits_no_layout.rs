use super::*;

#[test]
fn a_gift_ends_on_the_frame_clock_when_its_editor_commits_no_layout() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    let asked = host::now();
    assert_eq!(
        instances.expire_gifts(asked + gift::GIFT_WAIT * 2),
        None,
        "the clock does not run before the child is up"
    );
    let up = asked + gift::GIFT_WAIT * 2;
    instances.start_gift_count(INSTANCE, ChildId(1), up);
    assert_eq!(
        instances.expire_gifts(up),
        Some(gift::GIFT_WAIT),
        "the host is woken when the gift would end"
    );
    assert_eq!(
        instances.expire_gifts(up + gift::GIFT_WAIT / 2),
        Some(gift::GIFT_WAIT / 2)
    );
    assert!(instances.gift_pending(INSTANCE));
    assert_eq!(instances.expire_gifts(up + gift::GIFT_WAIT), None);
    assert!(
        !instances.gift_pending(INSTANCE),
        "an editor that commits nothing after its child is up still lets go of the keys"
    );
}
