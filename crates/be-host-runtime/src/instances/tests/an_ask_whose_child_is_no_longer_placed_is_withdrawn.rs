use super::*;

#[test]
fn an_ask_whose_child_is_no_longer_placed_is_withdrawn() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let (_mailbox, heard) = hearing();
    let mut asking = child(Rect::from_min_size(Pos2::ZERO, SIZE), false);
    asking.children[0].keyboard_ask = 1;
    instances.set_children(asking.clone());
    instances.hear_keyboard_asks(&[asking.clone()]);
    host::deliver();
    heard.borrow_mut().clear();

    // Still placed: the ask stands.
    instances.set_children(asking.clone());
    instances.hear_keyboard_asks(&[asking.clone()]);
    host::deliver();
    assert!(heard.borrow().is_empty(), "{:?}", heard.borrow());

    // The commit that places it no more withdraws the ask, so it can never fire later.
    let mut gone = asking;
    gone.children.clear();
    instances.set_children(gone.clone());
    instances.hear_keyboard_asks(&[gone]);
    host::deliver();
    assert!(
        matches!(
            heard.borrow().as_slice(),
            [InstanceEvent::KeyboardAsked(None)]
        ),
        "{:?}",
        heard.borrow()
    );
}
