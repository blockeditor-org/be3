use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{PointerLock, PointerLockHandle};

#[test]
fn a_pointer_lock_reports_motion_only_while_it_holds_the_pointer() {
    let _root = crate::reactive::RootScope::new();
    let moved = create_signal(Vec2::ZERO);

    let document = build_node(move || {
        let locked = create_signal(false);
        view! {
            <PointerLock
                locked={locked}
                on_motion={move |motion: Vec2| moved.update(|moved| *moved += motion)}
            >|_: PointerLockHandle|
                <Frame width=100.0 height=100.0 />
            </PointerLock>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(vec![Input::PointerMotion(Vec2::new(4.0, 0.0))]);
    assert_eq!(
        moved.get_untracked(),
        Vec2::ZERO,
        "motion should go nowhere while the pointer is free"
    );
    assert!(
        !harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .pointer_locked
    );

    harness.click(pos2(50.0, 50.0));
    assert!(
        harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .pointer_locked,
        "pressing the region should ask for the pointer"
    );

    harness.frame(vec![Input::PointerMotion(Vec2::new(4.0, -2.0))]);
    assert_eq!(
        moved.get_untracked(),
        Vec2::new(4.0, -2.0),
        "motion should reach the control holding the pointer"
    );

    harness.key(Key::Escape, Modifiers::NONE);
    assert!(
        !harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .pointer_locked,
        "escape should let the pointer go"
    );

    harness.frame(vec![Input::PointerMotion(Vec2::new(9.0, 9.0))]);
    assert_eq!(
        moved.get_untracked(),
        Vec2::new(4.0, -2.0),
        "motion after letting go should go nowhere again"
    );
}
