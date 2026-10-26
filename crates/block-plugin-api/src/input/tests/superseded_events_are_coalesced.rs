use super::*;

#[test]
fn superseded_events_are_coalesced() {
    let mut queued = Vec::new();
    let moved =
        |x: f32, y: f32, time_micros: u64| timed(Input::PointerMoved(Pos2::new(x, y)), time_micros);
    let pressed = timed(
        Input::PointerButton {
            pos: Pos2::new(3.0, 4.0),
            button: PointerButton::Primary,
            pressed: true,
        },
        3,
    );
    let wheel =
        |x: f32, y: f32| TimedInput::from(Input::Scroll(ScrollDelta::Pixels(Vec2::new(x, y))));
    let held = || TimedInput::from(Input::Modifiers(Modifiers::default()));
    for event in [
        moved(1.0, 2.0, 1),
        moved(3.0, 4.0, 2),
        pressed.clone(),
        moved(5.0, 6.0, 4),
        moved(7.0, 8.0, 5),
        wheel(1.0, 2.0),
        wheel(3.0, 4.0),
        held(),
        held(),
    ] {
        InputBatch::queue(&mut queued, input(event));
    }
    assert_eq!(queued.len(), 1);
    assert_eq!(
        queued.first().cloned(),
        Some(batch(
            vec![
                moved(3.0, 4.0, 2),
                pressed,
                moved(7.0, 8.0, 5),
                wheel(4.0, 6.0),
                held(),
            ],
            false
        ))
    );
}
