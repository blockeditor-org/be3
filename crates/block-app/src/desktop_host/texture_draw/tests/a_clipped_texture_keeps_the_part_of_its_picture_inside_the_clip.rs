use super::*;

#[test]
fn a_clipped_texture_keeps_the_part_of_its_picture_inside_the_clip() {
    let (rect, uv) =
        placed(&at([0.0, 0.0, 8.0, 4.0], [2.0, 0.0, 6.0, 100.0])).expect("it is in the clip");
    assert_eq!(rect, [2.0, 0.0, 6.0, 4.0]);
    assert_eq!(uv, [0.25, 0.0, 0.75, 1.0]);
    assert!(placed(&at([0.0, 0.0, 8.0, 4.0], [9.0, 0.0, 10.0, 4.0])).is_none());
    assert!(placed(&at([0.0, 0.0, 0.0, 4.0], [0.0, 0.0, 8.0, 8.0])).is_none());
}
