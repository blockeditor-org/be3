use super::*;

#[test]
fn a_commit_carrying_a_new_keyboard_ask_names_its_child_to_the_compositor() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let (_mailbox, heard) = hearing();
    let asking = |ask: u64| {
        let mut placements = child(Rect::from_min_size(Pos2::ZERO, SIZE), false);
        placements.children[0].keyboard_ask = ask;
        vec![placements]
    };
    let commit = |instances: &mut Instances, ask: u64| {
        for placements in asking(ask) {
            instances.set_children(placements);
        }
        instances.hear_keyboard_asks(&asking(ask));
    };

    // A child placed asking for nothing names nothing.
    commit(&mut instances, 0);
    host::deliver();
    assert!(heard.borrow().is_empty());

    // The commit that carries a higher ask names its child at once, whether its app runs or not.
    commit(&mut instances, 1);
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

    // The same ask carried again names nothing; a new one names it again.
    commit(&mut instances, 1);
    host::deliver();
    assert!(heard.borrow().is_empty(), "{:?}", heard.borrow());
    commit(&mut instances, 2);
    host::deliver();
    assert!(matches!(
        heard.borrow().as_slice(),
        [InstanceEvent::KeyboardAsked(Some(ChildId(1)))]
    ));
}
