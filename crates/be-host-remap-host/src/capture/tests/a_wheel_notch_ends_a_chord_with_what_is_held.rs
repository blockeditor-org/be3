use super::*;

#[test]
fn a_wheel_notch_ends_a_chord_with_what_is_held() {
    let mut capture = started();
    let inputs = [press("F20"), Input::Wheel { x: 0, y: -120 }];
    assert_eq!(caught(&mut capture, &inputs), vec![]);
    assert_eq!(
        capture.chord(),
        Some(&Chord {
            held: vec![key("F20")],
            trigger: Caught::Wheel(Wheel::Up),
        })
    );
    assert_eq!(
        caught(&mut capture, &[Input::Wheel { x: 0, y: -120 }]),
        vec![Input::Wheel { x: 0, y: -120 }],
        "once caught, the wheel scrolls again"
    );

    let mut capture = started();
    caught(&mut capture, &[Input::Wheel { x: 15, y: 0 }]);
    assert_eq!(
        capture.chord(),
        Some(&Chord {
            held: vec![],
            trigger: Caught::Wheel(Wheel::Right),
        }),
        "a part of a notch is enough to tell the way"
    );
}
