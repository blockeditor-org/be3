use super::*;

#[test]
fn a_held_slot_is_never_written() {
    let size = Size {
        width: 4,
        height: 4,
    };
    let mut ring = Ring::new(2);
    assert_eq!(ring.pick(size, &[0, 0]), Pick::Write(0));
    ring.written(0, size);
    assert_eq!(ring.hold_latest(), Some(0));
    assert_eq!(ring.pick(size, &[0, 0]), Pick::Write(1));
    ring.written(1, size);
    assert_eq!(ring.pick(size, &[0, 0]), Pick::Busy);
}
