use std::time::Duration;

use super::*;
use crate::geometry::pos2;

#[test]
fn a_band_released_while_moving_along_the_edge_carries_on_along_it() {
    let start = Instant::now();
    let frame = Duration::from_millis(16);
    let dimensions = vec2(1000.0, 600.0);
    let limit = |at: Pos2| pos2(at.x.max(0.0), at.y.clamp(0.0, 500.0));
    let mut band = Band::new(WINDOW_SPRING);
    band.grab(dimensions);
    band.stretch(pos2(0.0, 300.0), vec2(-100.0, 0.0), dimensions, true, start);
    band.stretch(
        pos2(0.0, 290.0),
        vec2(-100.0, 0.0),
        dimensions,
        true,
        start + frame,
    );
    band.release(start + frame);

    let mut position = pos2(0.0, 290.0);
    band.step(start + frame * 2, &mut position, limit, true);

    assert!(
        position.y < 290.0,
        "the window keeps moving up the edge it was pulled past: {position:?}"
    );
    assert!(
        band.offset.x > rubber_band(-100.0, dimensions.x),
        "while it springs back from that edge: {:?}",
        band.offset
    );

    for step in 3..200 {
        band.step(start + frame * step, &mut position, limit, true);
    }

    assert!(
        !band.moving(),
        "the window comes to rest: {:?}",
        band.offset
    );
    assert_eq!(position.x, 0.0);
    assert!(
        position.y < 280.0 && position.y > 200.0,
        "it glides a little way up, then stops: {position:?}"
    );
}
