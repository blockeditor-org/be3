use super::*;

#[test]
fn writing_prefers_a_slot_of_the_frame_size() {
    let small = Size {
        width: 4,
        height: 4,
    };
    let large = Size {
        width: 8,
        height: 8,
    };
    let mut ring = Ring::new(3);
    assert_eq!(ring.pick(small, &[0, 0, 0]), Pick::Write(0));
    ring.written(0, small);
    assert_eq!(ring.pick(large, &[0, 0, 0]), Pick::Write(1));
    ring.written(1, large);
    assert_eq!(ring.pick(large, &[0, 0, 0]), Pick::Write(2));
    ring.written(2, large);
    assert_eq!(ring.pick(small, &[0, 0, 0]), Pick::Write(0));
    ring.abandon(0);
    assert_eq!(ring.pick(large, &[0, 0, 0]), Pick::Write(1));
}
