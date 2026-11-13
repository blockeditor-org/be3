use super::*;

#[test]
fn the_placer_hears_whom_it_gives_the_keyboard_to() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let (_mailbox, heard) = hearing();
    tell(&mut instances, ShellMessage::FocusBlockChild([1; 16]));
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    host::deliver();
    assert!(
        heard.borrow().is_empty(),
        "no child shows the block until the layout commits"
    );
    instances.settle_gifts(&[INSTANCE]);
    host::deliver();
    assert!(
        matches!(
            heard.borrow().as_slice(),
            [InstanceEvent::KeyboardGift(Some(ChildId(1)))]
        ),
        "the child that shows it is named: {:?}",
        heard.borrow()
    );
    heard.borrow_mut().clear();
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    host::deliver();
    assert!(
        matches!(
            heard.borrow().as_slice(),
            [InstanceEvent::KeyboardGift(Some(ChildId(1)))]
        ),
        "as is a child asked for directly"
    );
}
