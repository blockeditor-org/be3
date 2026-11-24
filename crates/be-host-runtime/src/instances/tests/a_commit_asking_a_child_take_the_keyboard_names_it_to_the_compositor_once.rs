use super::*;

#[test]
fn a_commit_asking_a_child_take_the_keyboard_names_it_to_the_compositor_once() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let (_mailbox, heard) = hearing();
    let asking = |ask: u64| {
        let mut placements = child(Rect::from_min_size(Pos2::ZERO, SIZE), false);
        placements.children[0].keyboard_ask = ask;
        placements
    };

    // A child placed asking for nothing names nothing.
    instances.set_children(asking(0));
    host::deliver();
    assert!(heard.borrow().is_empty());

    // The commit that carries a higher ask names its child at once, whether its app runs or not.
    instances.set_children(asking(1));
    host::deliver();
    assert!(
        matches!(
            heard.borrow().as_slice(),
            [InstanceEvent::KeyboardAsked(Some(ChildId(1)))]
        ),
        "the child asked for is named: {:?}",
        heard.borrow()
    );
    heard.borrow_mut().clear();

    // The next commit withdraws an ask the compositor did not act on, and the same ask carried
    // again names nothing: nothing waits for the child.
    instances.withdraw_keyboard_asks();
    instances.set_children(asking(1));
    host::deliver();
    assert!(
        matches!(
            heard.borrow().as_slice(),
            [InstanceEvent::KeyboardAsked(None)]
        ),
        "{:?}",
        heard.borrow()
    );
    heard.borrow_mut().clear();

    // A new ask names it again.
    instances.withdraw_keyboard_asks();
    instances.set_children(asking(2));
    host::deliver();
    assert!(matches!(
        heard.borrow().as_slice(),
        [InstanceEvent::KeyboardAsked(Some(ChildId(1)))]
    ));
}
