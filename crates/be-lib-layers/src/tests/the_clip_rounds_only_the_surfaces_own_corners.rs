use super::*;

#[test]
fn the_clip_rounds_only_the_surfaces_own_corners() {
    let mut cut = layer(1, Pos2::ZERO, (100, 100));
    assert_eq!(cut.rounded(), [true; 4]);
    cut.clip = rect(0.0, 0.0, 100.0, 60.0);
    assert_eq!(cut.rounded(), [true, true, false, false]);
}
