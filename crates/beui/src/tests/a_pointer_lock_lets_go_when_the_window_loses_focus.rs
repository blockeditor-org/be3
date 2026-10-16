use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{PointerLock, PointerLockHandle};

#[test]
fn a_pointer_lock_lets_go_when_the_window_loses_focus() {
    let _root = crate::reactive::RootScope::new();
    let (changes, set_changes) = create_signal(Vec::new());

    let document = build(move || {
        let (locked, set_locked) = create_signal(false);
        view! {
            <PointerLock
                locked
                on_change={move |locked: bool| {
                    set_changes.update(|changes| changes.push(locked));
                    set_locked.set(locked);
                }}
            >
                |_: PointerLockHandle| <Frame width=100.0 height=100.0 />
            </PointerLock>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 50.0));
    assert!(harness.frame(Vec::new()).window.pointer_locked);

    harness.frame(vec![Event::Focus(false)]);
    assert_eq!(changes.get_untracked(), [true, false]);
    assert!(
        !harness.frame(Vec::new()).window.pointer_locked,
        "a window that lost the keyboard should not still hold the pointer"
    );
}
