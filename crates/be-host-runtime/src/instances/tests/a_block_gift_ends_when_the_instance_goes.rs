use super::*;

#[test]
fn a_block_gift_ends_when_the_instance_goes() {
    let mut instances = placed();
    tell(&mut instances, ShellMessage::FocusBlockChild([1; 16]));
    assert!(instances.child_focus_pending(INSTANCE));
    instances.remove(INSTANCE);
    assert!(
        !instances.child_focus_pending(INSTANCE),
        "an instance that is gone gives nothing"
    );
}
