use super::*;

#[test]
fn the_lockfile_numbers_a_frozen_type() {
    // schema.lock gives y id 1 and x id 2, though x comes first in the source: each key is the
    // id shifted past the two flags and the wire type (3, four bytes).
    let point = FrozenPoint {
        x: 1,
        y: 2,
        label: None,
    };
    let encoded = [0x43, 2, 0, 0, 0, 0x83, 0x01, 1, 0, 0, 0];
    assert_eq!(bytes(&point), encoded);
    assert_eq!(decode::<FrozenPoint>(&encoded), Ok(point));
    assert!(<FrozenPoint as Encode>::FROZEN);
    assert!(!<PersonV1 as Encode>::FROZEN);
}
