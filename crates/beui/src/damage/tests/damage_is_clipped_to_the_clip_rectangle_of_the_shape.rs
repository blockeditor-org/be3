use super::*;

#[test]
fn damage_is_clipped_to_the_clip_rectangle_of_the_shape() {
    let clip = rect(0.0, 0.0, 25.0, 25.0);
    let before = [clipped(rect(0.0, 0.0, 100.0, 100.0), clip)];
    let after = [clipped(rect(0.0, 0.0, 90.0, 90.0), clip)];

    assert_eq!(between(&before, &after), clip);
}
