use super::*;

#[test]
fn a_block_gift_becomes_a_gift_to_the_child_that_shows_its_block() {
    let mut instances = placed();
    instances.next_screens(PASS);
    tell(&mut instances, ShellMessage::FocusBlockChild([1; 16]));
    instances.settle_gifts(&[INSTANCE]);
    assert!(
        instances.gift_pending(INSTANCE),
        "no child shows the block yet"
    );
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    instances.settle_gifts(&[INSTANCE]);
    assert!(
        instances.gives_to(INSTANCE, ChildId(1)),
        "the child that shows the block is given the keyboard"
    );
    assert!(instances.take_gift(INSTANCE, ChildId(1), notes_view(), false, (0, PASS)));
    assert!(
        !instances.gift_pending(INSTANCE),
        "the gift is no longer pending once the child's view takes it"
    );
}
