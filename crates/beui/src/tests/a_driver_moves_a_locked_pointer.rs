use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{PointerLock, PointerLockHandle};

#[test]
fn a_driver_moves_a_locked_pointer() {
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
    let mut driven = stepped(crate::testing::Showing(document));

    driven.click(pos2(50.0, 50.0));
    driven.motion(vec2(6.0, -4.0));
    assert_eq!(
        moved.get_untracked(),
        Vec2::new(6.0, -4.0),
        "the motion reaches the control holding the pointer"
    );
    assert!(
        driven.ask(&["motion", "6"]).is_err(),
        "motion needs both DX and DY"
    );
    assert!(
        driven.ask(&["motion", "left", "4"]).is_err(),
        "motion takes numbers"
    );
}
