use super::*;

fn modes(harness: &mut Harness) -> (Vec<(i32, i32, bool)>, Vec<i32>) {
    let mut client = TestClient::connect(harness.compositor().server());
    client.bind_outputs();
    client.exchange(harness.compositor().server());
    let received = &client.received;
    (
        received
            .output_modes
            .iter()
            .map(|(_, width, height, current)| (*width, *height, *current))
            .collect(),
        received
            .output_scales
            .iter()
            .map(|(_, scale)| *scale)
            .collect(),
    )
}

#[test]
fn an_output_lists_only_its_current_mode_in_pixels() {
    let mut harness = Harness::new();
    let mut monitor = monitors(&[Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 720.0))]);
    monitor[0].scale = 1.5;
    harness.compositor().set_monitors(monitor.clone());
    harness.settle();
    assert_eq!(
        modes(&mut harness),
        (vec![(1650, 1080, true)], vec![2]),
        "the mode is the monitor's size in pixels at its fractional scale, and the integer scale rounds up for clients that know only that"
    );

    monitor[0].scale = 2.0;
    harness.compositor().set_monitors(monitor);
    harness.settle();
    assert_eq!(
        modes(&mut harness),
        (vec![(2200, 1440, true)], vec![2]),
        "a changed monitor lists only its new mode"
    );
}
