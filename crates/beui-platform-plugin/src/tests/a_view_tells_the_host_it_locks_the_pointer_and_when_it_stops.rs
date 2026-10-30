use super::*;

fn locking(step: Rc<Cell<u8>>) -> Windowed {
    Windowed::empty().hooked(move |context| match step.get() {
        0 => context.set_pointer_locked(true),
        1 => context.set_pointer_locked(false),
        2 => context.close_window(),
        _ => panic!("out of cheese"),
    })
}

#[test]
fn a_view_tells_the_host_it_locks_the_pointer_and_when_it_stops() {
    let step = Rc::new(Cell::new(0));
    let shown = whole(40.0, 30.0);
    let mut surface = launched(&EditorHost::default(), locking(step.clone()));
    let frame = one(&mut surface, &shown);
    assert!(frame.window.pointer_locked);
    assert_eq!(frame.stopped, None);

    step.set(1);
    assert!(!one(&mut surface, &shown).window.pointer_locked);

    step.set(2);
    assert_eq!(
        one(&mut surface, &shown).stopped,
        Some(block_plugin_api::Stopped::Closed),
        "an app that closed its window says so rather than going blank"
    );
    assert_eq!(
        one(&mut surface, &shown).stopped,
        Some(block_plugin_api::Stopped::Closed)
    );

    let mut surface = launched(&EditorHost::default(), locking(Rc::new(Cell::new(3))));
    assert_eq!(
        one(&mut surface, &shown).stopped,
        Some(block_plugin_api::Stopped::Failed(
            "out of cheese".to_owned()
        ))
    );
}
