use crate::outline::polygon;

#[test]
fn emboldening_keeps_a_sharp_spike_bounded() {
    let spike: &[[f32; 2]] = &[[0.0, 0.0], [2.0, 0.0], [1.0, 40.0]];
    let mut outline = polygon(&[spike]);
    outline.embolden(2.0);
    let moved = outline.corners().remove(0);
    for (before, after) in spike.iter().zip(&moved) {
        let distance = (after[0] - before[0]).hypot(after[1] - before[1]);
        assert!(
            distance <= 2.0 * 2.0,
            "a point of the spike at {before:?} moves to {after:?}, more than twice the strength away"
        );
    }
    let tip = moved[2];
    assert!(
        tip[1] <= 41.0 + 0.001,
        "the tip of a spike too sharp to widen moves only with the whole outline, to {tip:?}"
    );
}
