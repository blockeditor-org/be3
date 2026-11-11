use super::*;

#[test]
fn a_block_gift_becomes_a_gift_to_the_child_that_shows_its_block() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.shell_call(ShellCall::FocusBlockChild {
        reply: reply(next_op()),
        instance: INSTANCE,
        block: [1; 16],
    });
    instances.settle_block_focus();
    assert!(
        instances.child_focus_pending(INSTANCE),
        "no child shows the block yet"
    );
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    instances.settle_block_focus();
    assert!(
        instances.take_child_focus(INSTANCE, ChildId(1)),
        "the child that shows the block is given the keyboard"
    );
    assert!(
        !instances.child_focus_pending(INSTANCE),
        "the gift ends once the child's view takes it"
    );
}
