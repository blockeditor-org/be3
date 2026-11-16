use super::*;

/// An editor asks to give the keyboard to a child before it hears the child was replaced (a
/// joined editor's tab body shown again): the host answers that the gift missed, so the editor
/// asks again for the child placed now, and makes no gift of it. A child placed again is given
/// it as any is.
#[test]
fn a_gift_asked_for_a_child_already_replaced_is_answered_that_it_missed() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    instances.set_children(ChildPlacements {
        children: Vec::new(),
        ..child(Rect::ZERO, false)
    });
    instances.pending();
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    let answered = instances.pending();
    assert!(
        answered.iter().any(|message| matches!(
            message,
            Outbound::Event(INSTANCE, be_sdk_api::app::EditorEvent::GiftMissed(ChildId(1)))
        )),
        "the editor hears its gift missed: {answered:?}"
    );
    assert!(!instances.gift_pending(INSTANCE), "no gift is made of it");

    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    assert!(
        instances.gives_to(INSTANCE, ChildId(1)),
        "placed again, it is given the keyboard"
    );
}
