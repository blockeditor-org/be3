use super::*;

#[test]
fn a_gift_ends_on_the_frame_clock_when_its_editor_commits_no_layout() {
    let mut instances = placed();
    instances.next_screens(PASS);
    tell(&mut instances, ShellMessage::FocusBlockChild([3; 16]));
    let asked = host::now();
    assert_eq!(
        instances.expire_gifts(asked),
        Some(gift::GIFT_WAIT),
        "the host is woken when the gift would end"
    );
    assert_eq!(
        instances.expire_gifts(asked + gift::GIFT_WAIT / 2),
        Some(gift::GIFT_WAIT / 2)
    );
    assert!(instances.gift_pending(INSTANCE));
    assert_eq!(instances.expire_gifts(asked + gift::GIFT_WAIT), None);
    assert!(
        !instances.gift_pending(INSTANCE),
        "an editor that commits nothing after asking still lets go of the keys"
    );
}
