use super::*;

#[test]
fn a_hold_begins_and_ends_once() {
    let mut touchpad = Touchpad::default();
    let held = [
        TouchPhase::Started,
        TouchPhase::Started,
        TouchPhase::Ended,
        TouchPhase::Ended,
    ]
    .into_iter()
    .filter_map(|phase| touchpad.event(Gesture::Hold, phase))
    .collect::<Vec<_>>();
    assert_eq!(
        held,
        [
            TouchpadGesture::HoldBegan(2),
            TouchpadGesture::HoldEnded(false)
        ]
    );
}
