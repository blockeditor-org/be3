use super::*;

use beui_types::{TouchPhase, WindowId};

fn touch_id(finger: u64) -> TouchId {
    TouchId { device: 0, finger }
}

fn touch(id: u64, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch {
        id: touch_id(id),
        phase,
        pos,
        force: None,
    }
}

/// Fingers on two touch displays at once are two gestures: the second window's first finger
/// is a first press there, not a second finger of the first window's, nor the second tap of
/// a double-tap, and cancelling them reaches each in its own window.
#[test]
fn a_finger_in_another_window_never_joins_the_first_windows_gesture() {
    let now = Instant::now();
    let at = Pos2::new(40.0, 30.0);
    let (first, second) = (WindowId::next(), WindowId::next());

    let mut input = InputState::default();
    input.begin_frame(
        vec![Event::PointerWindow(first), touch(1, TouchPhase::Start, at)],
        now,
    );
    input.begin_frame(vec![touch(1, TouchPhase::End, at)], now);
    input.begin_frame(vec![touch(1, TouchPhase::Start, at)], now);
    assert_eq!(
        input.pointer.clicks(),
        2,
        "a second tap in one window is a double-tap"
    );

    input.begin_frame(
        vec![
            Event::PointerWindow(second),
            touch(2, TouchPhase::Start, at),
        ],
        now,
    );
    let down = input.pointer_events[0];
    assert_eq!(
        (down.window, down.clicks),
        (second, 1),
        "a finger in another window joined the first window's taps"
    );

    input.begin_frame(
        vec![
            Event::PointerWindow(second),
            touch(1, TouchPhase::Move, Pos2::new(50.0, 30.0)),
        ],
        now,
    );
    assert_eq!(
        input.pointer_events[0].window, first,
        "a finger's move was read in the window the last event named"
    );

    input.begin_frame(vec![Event::PointersCancelled], now);
    let cancelled: Vec<(PointerId, beui_types::WindowId)> = input
        .pointer_events
        .iter()
        .filter(|event| event.phase == PointerPhase::Cancel)
        .map(|event| (event.pointer, event.window))
        .collect();
    assert_eq!(
        cancelled,
        [
            (PointerId::Touch(touch_id(1)), first),
            (PointerId::Touch(touch_id(2)), second)
        ]
    );
}
