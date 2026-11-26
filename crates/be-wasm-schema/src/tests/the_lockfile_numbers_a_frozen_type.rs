use super::*;

#[test]
fn the_lockfile_numbers_a_frozen_type() {
    // schema.lock gives y id 1 and x id 2, though x comes first in the source: each key is the
    // id shifted past the wire type (2, four bytes).
    let point = FrozenPoint {
        x: 1,
        y: 2,
        label: None,
    };
    let bytes = [0x0a, 2, 0, 0, 0, 0x12, 1, 0, 0, 0];
    assert_eq!(encode(&point), bytes);
    assert_eq!(decode::<FrozenPoint>(&bytes), Ok(point));
}
