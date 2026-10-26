use super::*;
use crate::carry::STEP_PER_LINE;
use beui_core::runner::SCROLL_LINE;

#[test]
fn a_wheel_line_moves_a_carried_panel_a_step_and_pixels_in_proportion() {
    let (mut space, now) = space();
    send(
        &mut space,
        now,
        [Input::PointerMoved(centre()), button(centre(), true)],
    );
    space.begin_carry(Pointer::Mouse, WindowId::MAIN);
    let distance = |space: &Space| space.mouse.carry.as_ref().unwrap().distance;
    let start = distance(&space);

    send(&mut space, now, [scroll(1.0)]);
    let line = distance(&space) / start;
    assert!(
        (line - 1.0 / STEP_PER_LINE).abs() < 1e-4,
        "a line is one step: {line}"
    );
    assert!(line < 1.06, "a small one: {line}");

    let before = distance(&space);
    let mut seen = vec![before];
    for _ in 0..10 {
        send(
            &mut space,
            now,
            [Input::Scroll(ScrollDelta::Pixels(vec2(
                0.0,
                SCROLL_LINE / 10.0,
            )))],
        );
        seen.push(distance(&space));
    }
    let steps: Vec<f32> = seen.windows(2).map(|pair| pair[1] / pair[0]).collect();
    assert!(
        steps
            .iter()
            .all(|step| (step - STEP_PER_LINE.powf(-0.1)).abs() < 1e-4),
        "each touchpad delta moves it by its own share of a line: {steps:?}"
    );
    let whole = distance(&space) / before;
    assert!(
        (whole - 1.0 / STEP_PER_LINE).abs() < 1e-3,
        "and a line's worth of pixels as far as a line: {whole}"
    );
}
