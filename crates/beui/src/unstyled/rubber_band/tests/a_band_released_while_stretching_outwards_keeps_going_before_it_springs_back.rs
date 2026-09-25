use std::time::Duration;

use super::*;

fn released(moving: bool) -> Band {
    let start = Instant::now();
    let frame = Duration::from_millis(16);
    let dimensions = vec2(1000.0, 600.0);
    let mut band = Band::new(WINDOW_SPRING);
    band.grab(dimensions);
    band.stretch(vec2(-100.0, 0.0), dimensions, true, start);
    let pulled = if moving { -160.0 } else { -100.0 };
    band.stretch(vec2(pulled, 0.0), dimensions, true, start + frame);
    band.release(start + frame);
    band.step(start + frame * 2);
    band
}

#[test]
fn a_band_released_while_stretching_outwards_keeps_going_before_it_springs_back() {
    let dimensions = vec2(1000.0, 600.0);
    let flung = released(true);
    let held = released(false);

    assert!(
        flung.offset.x < rubber_band(-160.0, dimensions.x),
        "a band let go while still being pulled carries on outwards: {:?}",
        flung.offset
    );
    assert!(
        held.offset.x > rubber_band(-100.0, dimensions.x),
        "a band let go while held still heads straight back: {:?}",
        held.offset
    );
    assert!(flung.moving() && held.moving());
}
