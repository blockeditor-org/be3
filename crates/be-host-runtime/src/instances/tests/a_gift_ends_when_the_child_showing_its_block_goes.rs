use super::*;

#[test]
fn a_gift_ends_when_the_child_showing_its_block_goes() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let (_mailbox, heard) = hearing();
    tell(&mut instances, ShellMessage::FocusBlockChild([1; 16]));
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    instances.settle_gifts(&[INSTANCE]);
    assert!(instances.gives_to(INSTANCE, ChildId(1)));
    instances.set_children(ChildPlacements {
        children: Vec::new(),
        ..child(Rect::ZERO, false)
    });
    instances.settle_gifts(&[INSTANCE]);
    assert!(
        !instances.gift_pending(INSTANCE),
        "no child shows the block any more"
    );
    host::deliver();
    assert!(
        matches!(
            heard.borrow().as_slice(),
            [
                InstanceEvent::KeyboardGift(Some(ChildId(1))),
                InstanceEvent::KeyboardGift(None)
            ]
        ),
        "its views hear it ended: {:?}",
        heard.borrow()
    );
}
