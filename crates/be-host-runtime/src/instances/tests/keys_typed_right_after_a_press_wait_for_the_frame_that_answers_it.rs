use super::*;

#[test]
fn keys_typed_right_after_a_press_wait_for_the_frame_that_answers_it() {
    let mut instances = placed();
    instances.next_screens(PASS);
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    let pass = host::pass();
    let app = |asked, answered| progress((asked, answered), true, false);
    assert_eq!(
        own_keys(&mut instances, app(3, 3), true, vec![press()], pass).len(),
        1,
        "the press goes through"
    );
    instances.sent_press(INSTANCE, own_view(), 3);
    assert_eq!(
        own_keys(&mut instances, app(4, 3), true, vec![letter("T")], pass),
        [],
        "a key typed before the editor drew the frame that took the press waits for it"
    );
    tell(&mut instances, ShellMessage::FocusChild(ChildId(1)));
    assert_eq!(
        own_keys(&mut instances, app(4, 4), true, vec![letter("h")], pass),
        [],
        "that frame asked to give the keyboard to a child, so the keys are its"
    );
    assert!(instances.take_gift(INSTANCE, ChildId(1), notes_view(), false, (1, pass)));
    assert_eq!(
        notes_keys(&mut instances, progress((1, 1), true, true), true, Vec::new(), pass),
        [],
        "the child's field asked for an input method before it had the focus"
    );
    assert_eq!(
        notes_keys(&mut instances, progress((2, 2), true, true), true, Vec::new(), pass),
        [letter("T"), letter("h")]
    );

    assert_eq!(
        own_keys(&mut instances, app(5, 5), true, vec![press()], pass).len(),
        1
    );
    instances.sent_press(INSTANCE, own_view(), 5);
    assert_eq!(
        own_keys(&mut instances, app(6, 5), true, vec![letter("x")], pass),
        []
    );
    assert_eq!(
        instances.settle_own_keys(INSTANCE, &own_view(), app(6, 6), true, pass),
        Some(beui::Modifiers::NONE),
        "a press whose frame asked for nothing gives the editor its keys as soon as it is drawn"
    );
    assert_eq!(
        own_keys(&mut instances, app(6, 6), true, Vec::new(), pass),
        [letter("x")]
    );
}
