use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_second_finger_cancels_the_press_the_first_began() {
    let _root = crate::reactive::RootScope::new();
    let cancelled = create_signal(0);
    let clicked = create_signal(false);
    let document = build_node(move || {
        view! {
            <Interactive
                on_cancel={move || cancelled.update(|cancelled| *cancelled += 1)}
                on_click={move || clicked.set(true)}
            >
                <Frame width=300.0 height=300.0 />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.finger(1, TouchPhase::Start, pos2(20.0, 20.0));
    harness.finger(1, TouchPhase::End, pos2(20.0, 20.0));
    assert_eq!(cancelled.get_untracked(), 0, "a tap is not cancelled");
    assert!(clicked.update(|clicked| std::mem::replace(clicked, false)));

    harness.finger(1, TouchPhase::Start, pos2(100.0, 100.0));
    harness.finger(2, TouchPhase::Start, pos2(200.0, 200.0));
    harness.finger(2, TouchPhase::End, pos2(200.0, 200.0));
    harness.finger(1, TouchPhase::End, pos2(100.0, 100.0));

    assert_eq!(
        cancelled.get_untracked(),
        1,
        "the second finger cancels the press once"
    );
    assert!(!clicked.get_untracked(), "a cancelled press is not a click");
}
