use super::*;

#[test]
fn a_slot_released_again_before_its_signal_ran_owes_one_signal() {
    let size = Size {
        width: 4,
        height: 4,
    };
    let mut ring = Ring::new(2);
    assert_eq!(ring.pick(size, &[0, 0]), Pick::Write(0));
    ring.written(0, size);
    ring.hold_latest();
    assert_eq!(ring.release(0, 0), Some(1));
    ring.hold_latest();
    ring.hold_latest();
    assert_eq!(ring.release(0, 0), None);
    assert_eq!(ring.release(0, 0), None);
    assert_eq!(ring.settle(&[0, 0]), Vec::new());
    assert_eq!(ring.pick(size, &[0, 0]), Pick::Write(1));
    ring.written(1, size);
    assert_eq!(ring.pick(size, &[0, 0]), Pick::Wait(vec![(0, 1)]));
    assert_eq!(ring.settle(&[1, 0]), vec![(0, 2)]);
    assert_eq!(ring.pick(size, &[1, 0]), Pick::Wait(vec![(0, 2)]));
    assert_eq!(ring.pick(size, &[2, 0]), Pick::Write(0));
    assert_eq!(ring.release_value(0), 2);
}
