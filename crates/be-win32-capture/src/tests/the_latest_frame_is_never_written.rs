use super::*;

#[test]
fn the_latest_frame_is_never_written() {
    let size = Size {
        width: 4,
        height: 4,
    };
    let mut ring = Ring::new(1);
    assert_eq!(ring.pick(size, &[0]), Pick::Write(0));
    ring.written(0, size);
    assert_eq!(ring.latest(), Some(0));
    assert_eq!(ring.pick(size, &[0]), Pick::Busy);
}
