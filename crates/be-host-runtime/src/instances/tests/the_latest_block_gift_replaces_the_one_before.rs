use super::*;

#[test]
fn the_latest_block_gift_replaces_the_one_before() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let ask = |instances: &mut Instances, block: [u8; 16]| {
        tell(instances, ShellMessage::FocusBlockChild(block));
    };
    ask(&mut instances, [2; 16]);
    ask(&mut instances, [1; 16]);
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    instances.settle_block_focus();
    assert!(
        instances.take_child_focus(INSTANCE, ChildId(1)),
        "the child that shows the block asked for last is given the keyboard"
    );
    instances.settle_block_focus();
    assert!(
        !instances.child_focus_pending(INSTANCE),
        "the earlier ask was replaced, so nothing waits for a block no child shows"
    );
}
