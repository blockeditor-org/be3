use super::*;

#[test]
fn the_lockfile_numbers_a_frozen_type() {
    // schema.lock gives y id 1 and x id 2, though x comes first in the source.
    let point = FrozenPoint {
        x: 1,
        y: 2,
        label: None,
    };
    assert_eq!(encode(&point), [0x08, 4, 0x10, 2]);
    assert_eq!(decode::<FrozenPoint>(&[0x08, 4, 0x10, 2]), Ok(point));
}
