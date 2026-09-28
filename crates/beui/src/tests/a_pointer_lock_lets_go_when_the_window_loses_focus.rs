use super::*;
use crate::reactive::{Frame, build, create_signal, view};
use crate::unstyled::{PointerLock, PointerLockHandle};

#[test]
fn a_pointer_lock_lets_go_when_the_window_loses_focus() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();

    let document = build(move || {
        let (locked, set_locked) = create_signal(false);
        view! {
            <PointerLock
                locked
                on_change={move |locked: bool| {
                    sink.borrow_mut().push(locked);
                    set_locked.set(locked);
                }}
            >
                {move |_: PointerLockHandle| view! {
                    <Frame width=100.0 height=100.0 />
                }}
            </PointerLock>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 50.0));
    assert!(harness.frame(Vec::new()).pointer_locked);

    harness.frame(vec![Event::Focus(false)]);
    assert_eq!(*changes.borrow(), [true, false]);
    assert!(
        !harness.frame(Vec::new()).pointer_locked,
        "a window that lost the keyboard should not still hold the pointer"
    );
}
