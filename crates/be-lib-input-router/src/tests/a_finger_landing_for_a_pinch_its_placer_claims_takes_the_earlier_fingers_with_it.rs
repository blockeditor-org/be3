use super::*;

#[test]
fn a_finger_landing_for_a_pinch_its_placer_claims_takes_the_earlier_fingers_with_it() {
    let test = canvas_with_editor();
    let mut router = Router::new(ROOT);
    router.route(&test, touch(1, TouchPhase::Start, 250.0, 250.0));
    let landed = router.route(&test, touch(2, TouchPhase::Start, 150.0, 150.0));
    assert_eq!(
        landed,
        vec![
            Routed::Input {
                target: EDITOR,
                input: touch(1, TouchPhase::Cancel, 50.0, 50.0),
            },
            Routed::Input {
                target: CANVAS,
                input: touch(1, TouchPhase::Start, 150.0, 150.0),
            },
            Routed::Input {
                target: CANVAS,
                input: touch(2, TouchPhase::Start, 50.0, 50.0),
            },
        ],
        "a pinch with one finger on the canvas and one on the editor placed on it is the canvas's"
    );
    assert_eq!(
        targets(&router.route(&test, touch(1, TouchPhase::Move, 260.0, 260.0))),
        vec![CANVAS]
    );
}
