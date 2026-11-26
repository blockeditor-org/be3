use super::*;

#[test]
fn an_ask_after_a_restart_counts_afresh() {
    let mut instances = placed();
    instances.next_screens(PASS);
    let (_mailbox, heard) = hearing();
    let asking = |ask: u64| {
        let mut placements = child(Rect::from_min_size(Pos2::ZERO, SIZE), false);
        placements.children[0].keyboard_ask = ask;
        vec![placements]
    };
    for placements in asking(5) {
        instances.set_children(placements);
    }
    instances.hear_keyboard_asks(&asking(5));
    host::deliver();
    heard.borrow_mut().clear();

    // The new run's editor counts its asks from 1 again; the old run's ask is withdrawn.
    restart(&mut instances);
    host::deliver();
    assert!(
        heard
            .borrow()
            .iter()
            .any(|event| matches!(event, InstanceEvent::KeyboardAsked(None))),
        "the old run's ask is withdrawn: {:?}",
        heard.borrow()
    );
    heard.borrow_mut().clear();
    instances.next_screens(PASS);
    for placements in asking(1) {
        instances.set_children(placements);
    }
    instances.hear_keyboard_asks(&asking(1));
    host::deliver();
    assert!(
        heard
            .borrow()
            .iter()
            .any(|event| matches!(event, InstanceEvent::KeyboardAsked(Some(ChildId(1))))),
        "the new run's first ask names its child: {:?}",
        heard.borrow()
    );
}
