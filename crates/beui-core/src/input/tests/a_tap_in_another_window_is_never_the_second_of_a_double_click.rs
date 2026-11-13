use super::*;

use beui_types::input::{Pen, PenPhase};
use beui_types::{PointerButton, WindowId};

fn pressed(input: &mut InputState, window: WindowId, press: Event, now: Instant) -> u32 {
    input.begin_frame(vec![Event::PointerWindow(window), press], now);
    input.pointer.clicks()
}

/// A press counts as a repeat only in the window of the one before, whatever kind of press it
/// is: the pen's is counted before its position is placed, which once read the window it left.
#[test]
fn a_tap_in_another_window_is_never_the_second_of_a_double_click() {
    let now = Instant::now();
    let at = Pos2::new(40.0, 30.0);
    let (first, second) = (WindowId::next(), WindowId::next());
    let pen = |phase| Event::Pen {
        phase,
        pos: at,
        pen: Pen::default(),
    };
    let down = PenPhase::Down(PointerButton::Primary);
    let up = PenPhase::Up(PointerButton::Primary);

    let mut input = InputState::default();
    assert_eq!(pressed(&mut input, first, pen(down), now), 1);
    input.begin_frame(vec![pen(up)], now);
    assert_eq!(
        pressed(&mut input, second, pen(down), now),
        1,
        "a pen tap in another window counted as a double-click"
    );
    input.begin_frame(vec![pen(up)], now);
    assert_eq!(
        pressed(&mut input, second, pen(down), now),
        2,
        "a second tap in the same window is one"
    );

    let mouse = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::default(),
    };
    let mut input = InputState::default();
    assert_eq!(pressed(&mut input, first, mouse(true), now), 1);
    input.begin_frame(vec![mouse(false)], now);
    assert_eq!(
        pressed(&mut input, second, mouse(true), now),
        1,
        "a click in another window counted as a double-click"
    );
}
