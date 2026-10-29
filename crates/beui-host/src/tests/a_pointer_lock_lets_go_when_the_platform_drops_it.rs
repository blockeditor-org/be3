use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{PointerLock, PointerLockHandle};

#[test]
fn a_pointer_lock_lets_go_when_the_platform_drops_it() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());

    let document = build_node(move || {
        let locked = create_signal(false);
        view! {
            <PointerLock
                locked={Binding::new(locked, move |next: bool| {
                    changes.update(|changes| changes.push(next));
                    locked.set(next);
                })}
            >|_: PointerLockHandle|
                <Frame width=100.0 height=100.0 />
            </PointerLock>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(50.0, 50.0));
    assert!(
        harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .pointer_locked
    );

    harness.frame(vec![Input::PointerUnlocked]);
    assert_eq!(changes.get_untracked(), [true, false]);
    assert!(
        !harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .pointer_locked,
        "a control whose lock the platform dropped (a browser on Escape) should stop asking for it"
    );
}
