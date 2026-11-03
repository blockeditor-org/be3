use super::*;

#[test]
fn a_pinch_reports_its_spread_from_the_start_and_its_pan_and_turn_since_the_last_update() {
    let mut touchpad = Touchpad::default();
    let frame = |touchpad: &mut Touchpad, phase, pan, pinch, rotation| {
        [
            Gesture::Pan(pan),
            Gesture::Pinch(pinch),
            Gesture::Rotation(rotation),
        ]
        .into_iter()
        .filter_map(|gesture| touchpad.event(gesture, phase))
        .collect::<Vec<_>>()
    };
    assert_eq!(
        frame(&mut touchpad, TouchPhase::Started, vec2(0.0, 0.0), 0.0, 0.0),
        [TouchpadGesture::PinchBegan(2)],
        "the three events winit sends for a pinch beginning begin it once"
    );
    assert_eq!(
        frame(
            &mut touchpad,
            TouchPhase::Moved,
            vec2(3.0, -2.0),
            0.25,
            -10.0
        ),
        [
            TouchpadGesture::Pinched {
                delta: vec2(3.0, -2.0),
                scale: 1.25,
                rotation: 0.0,
            },
            TouchpadGesture::Pinched {
                delta: vec2(0.0, 0.0),
                scale: 1.25,
                rotation: 10.0,
            },
        ],
        "the pan goes with the spread, and winit's counterclockwise turn is reported clockwise"
    );
    assert_eq!(
        frame(&mut touchpad, TouchPhase::Moved, vec2(1.0, 1.0), -0.5, 0.0),
        [TouchpadGesture::Pinched {
            delta: vec2(1.0, 1.0),
            scale: 0.75,
            rotation: 0.0,
        }],
        "the spread adds up from the start, and no turn sends nothing more"
    );
    assert_eq!(
        frame(
            &mut touchpad,
            TouchPhase::Cancelled,
            vec2(0.0, 0.0),
            0.0,
            0.0
        ),
        [TouchpadGesture::PinchEnded(true)],
        "the pinch ends once, cancelled"
    );
}
