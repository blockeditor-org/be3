use super::*;
use crate::reactive::{Frame, build, create_signal, view};
use crate::unstyled::{PointerLock, PointerLockHandle};

#[test]
fn a_pointer_lock_reports_motion_only_while_it_holds_the_pointer() {
    let moved = Rc::new(Cell::new(Vec2::ZERO));
    let sink = moved.clone();

    let document = build(move || {
        let (locked, set_locked) = create_signal(false);
        view! {
            <PointerLock
                locked
                on_change={move |locked: bool| set_locked.set(locked)}
                on_motion={move |motion: Vec2| sink.set(sink.get() + motion)}
            >
                {move |_: PointerLockHandle| view! {
                    <Frame width=100.0 height=100.0 />
                }}
            </PointerLock>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(vec![Event::PointerMotion(Vec2::new(4.0, 0.0))]);
    assert_eq!(
        moved.get(),
        Vec2::ZERO,
        "motion should go nowhere while the pointer is free"
    );
    assert!(!harness.frame(Vec::new()).pointer_locked);

    harness.click(pos2(50.0, 50.0));
    assert!(
        harness.frame(Vec::new()).pointer_locked,
        "pressing the region should ask for the pointer"
    );

    harness.frame(vec![Event::PointerMotion(Vec2::new(4.0, -2.0))]);
    assert_eq!(
        moved.get(),
        Vec2::new(4.0, -2.0),
        "motion should reach the control holding the pointer"
    );

    harness.key(Key::Escape, Modifiers::NONE);
    assert!(
        !harness.frame(Vec::new()).pointer_locked,
        "escape should let the pointer go"
    );

    harness.frame(vec![Event::PointerMotion(Vec2::new(9.0, 9.0))]);
    assert_eq!(
        moved.get(),
        Vec2::new(4.0, -2.0),
        "motion after letting go should go nowhere again"
    );
}
