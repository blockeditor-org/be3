use super::*;

#[test]
fn a_block_gift_ends_when_no_child_shows_its_block() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let (_mailbox, heard) = hearing();
    tell(&mut instances, ShellMessage::FocusBlockChild([3; 16]));
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    for _ in 1..gift::GIFT_COMMITS {
        instances.settle_gifts(&[INSTANCE]);
    }
    assert!(instances.gift_pending(INSTANCE));
    instances.settle_gifts(&[INSTANCE]);
    assert!(
        !instances.gift_pending(INSTANCE),
        "the editor laid itself out again and again and no child it places shows the block"
    );
    host::deliver();
    assert!(
        matches!(
            heard.borrow().as_slice(),
            [InstanceEvent::KeyboardGift(None)]
        ),
        "its views hear it ended: {:?}",
        heard.borrow()
    );
}
