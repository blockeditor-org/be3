use super::*;

#[test]
fn superseded_events_are_coalesced() {
    let mut session = running_session();
    let moved = |x: f32, y: f32, time_micros: u64| InputEvent::PointerMoved { x, y, time_micros };
    let pressed = InputEvent::PointerButton {
        button: PointerButton::Primary,
        pressed: true,
        x: 3.0,
        y: 4.0,
        time_micros: 3,
    };
    let wheel = |x: f32, y: f32| InputEvent::Wheel {
        x,
        y,
        unit: WheelUnit::Pixels,
    };
    for event in [
        moved(1.0, 2.0, 1),
        moved(3.0, 4.0, 2),
        pressed.clone(),
        moved(5.0, 6.0, 4),
        moved(7.0, 8.0, 5),
        wheel(1.0, 2.0),
        wheel(3.0, 4.0),
        InputEvent::Modifiers(Modifiers::default()),
        InputEvent::Modifiers(Modifiers::default()),
    ] {
        session.enqueue(input(event)).unwrap();
    }
    assert_eq!(session.queued_message_count(), 1);
    assert_eq!(
        session.next_outbound(),
        Some(batch(
            vec![
                moved(3.0, 4.0, 2),
                pressed,
                moved(7.0, 8.0, 5),
                wheel(4.0, 6.0),
                InputEvent::Modifiers(Modifiers::default()),
            ],
            false
        ))
    );
}
