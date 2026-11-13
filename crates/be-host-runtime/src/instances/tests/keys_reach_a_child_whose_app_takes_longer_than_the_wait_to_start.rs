use super::*;

#[test]
fn keys_reach_a_child_whose_app_takes_longer_than_the_wait_to_start() {
    let mut instances = placed();
    instances.next_screens(PASS);
    tell(&mut instances, ShellMessage::FocusBlockChild([1; 16]));
    let pass = host::pass();
    let asked = host::now();
    own_keys(
        &mut instances,
        progress((4, 4), true, false),
        true,
        vec![letter("T"), letter("h")],
        pass,
    );
    instances.set_children(child(Rect::from_min_size(Pos2::ZERO, SIZE), false));
    for _ in 0..gift::GIFT_COMMITS * 2 {
        instances.settle_gifts(&[INSTANCE]);
    }
    let started = asked + gift::GIFT_WAIT * 3;
    instances.expire_gifts(started);
    assert!(
        instances.gives_to(INSTANCE, ChildId(1)),
        "the notes' app is still downloading and compiling, long after the ask"
    );
    instances.start_gift_count(INSTANCE, ChildId(1), started);
    assert!(instances.take_gift(INSTANCE, ChildId(1), notes_view(), false, (0, pass)));
    assert_eq!(
        notes_keys(
            &mut instances,
            progress((1, 1), true, true),
            true,
            Vec::new(),
            pass
        ),
        []
    );
    assert_eq!(
        notes_keys(
            &mut instances,
            progress((2, 2), true, true),
            true,
            Vec::new(),
            pass
        ),
        [letter("T"), letter("h")],
        "what was typed while it started reaches the notes, not the slide"
    );
}
