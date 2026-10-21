use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_quick_tap_with_several_fingers_is_a_finger_tap() {
    let _root = crate::reactive::RootScope::new();
    let taps = create_signal::<Vec<usize>>(Vec::new());
    let document = build_node(move || {
        on_finger_tap(move |fingers: usize| {
            taps.update(|taps| taps.push(fingers));
            true
        });
        view! {
            <Frame width=300.0 height=300.0 />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.touch(TouchPhase::Start, pos2(50.0, 50.0));
    harness.touch(TouchPhase::End, pos2(50.0, 50.0));
    assert!(
        taps.with_untracked(|taps| taps.is_empty()),
        "one finger is an ordinary tap"
    );

    harness.finger(1, TouchPhase::Start, pos2(50.0, 50.0));
    harness.finger(2, TouchPhase::Start, pos2(120.0, 50.0));
    harness.finger(3, TouchPhase::Start, pos2(190.0, 50.0));
    harness.finger(3, TouchPhase::End, pos2(190.0, 50.0));
    harness.finger(2, TouchPhase::End, pos2(120.0, 50.0));
    harness.finger(1, TouchPhase::End, pos2(50.0, 50.0));
    assert_eq!(
        taps.get_untracked(),
        vec![3],
        "the tap counts the most fingers that were down"
    );

    harness.finger(1, TouchPhase::Start, pos2(50.0, 50.0));
    harness.finger(2, TouchPhase::Start, pos2(120.0, 50.0));
    harness.move_fingers(pos2(30.0, 50.0), pos2(160.0, 50.0));
    harness.finger(1, TouchPhase::End, pos2(30.0, 50.0));
    harness.finger(2, TouchPhase::End, pos2(160.0, 50.0));
    assert_eq!(
        taps.get_untracked(),
        vec![3],
        "fingers that moved were a pinch, not a tap"
    );
}
