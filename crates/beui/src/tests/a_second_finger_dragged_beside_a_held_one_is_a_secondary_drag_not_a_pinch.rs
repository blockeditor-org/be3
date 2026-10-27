use super::*;
use crate::input::{SecondaryDrag, ZoomGesture};
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_second_finger_dragged_beside_a_held_one_is_a_secondary_drag_not_a_pinch() {
    let _root = crate::reactive::RootScope::new();
    let drags = create_signal::<Vec<SecondaryDrag>>(Vec::new());
    let zoomed = create_signal(false);
    let document = build_node(move || {
        let pinch =
            crate::unstyled::gestures::pan_zoom(Callback::empty(), move |_: ZoomGesture| {
                zoomed.set(true)
            });
        view! {
            <Interactive
                on_pointer={crate::unstyled::gestures::first_of([
                    crate::unstyled::gestures::secondary_drag(move |drag| drags.update(|drags| drags.push(drag))),
                    pinch.pointer,
                ])}
                on_wheel={pinch.wheel}
            >
                <Frame width=300.0 height=300.0 />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.finger(1, TouchPhase::Start, pos2(20.0, 20.0));
    harness.finger(2, TouchPhase::Start, pos2(100.0, 100.0));
    harness.finger(2, TouchPhase::Move, pos2(200.0, 150.0));
    harness.finger(2, TouchPhase::End, pos2(250.0, 160.0));
    harness.finger(1, TouchPhase::End, pos2(20.0, 20.0));
    harness.frame(Vec::new());

    assert!(!zoomed.get_untracked(), "a held finger does not pinch");
    let seen = drags.get_untracked();
    let last = seen.last().expect("the drag was reported");
    assert!(seen[0].started);
    assert_eq!(last.from, pos2(100.0, 100.0));
    assert_eq!(last.pos, pos2(250.0, 160.0));
    assert!(last.ended && !last.cancelled);

    drop(seen);
    harness.finger(1, TouchPhase::Start, pos2(20.0, 20.0));
    harness.finger(2, TouchPhase::Start, pos2(100.0, 100.0));
    harness.move_fingers(pos2(0.0, 0.0), pos2(150.0, 150.0));
    harness.frame(Vec::new());

    assert!(
        zoomed.get_untracked(),
        "moving the first finger too makes it a pinch"
    );
    let cancelled = drags
        .get_untracked()
        .last()
        .is_some_and(|drag| drag.cancelled);
    assert!(cancelled, "the drag the second finger began is called off");
}
