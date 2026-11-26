use super::*;

#[test]
fn malformed_input_is_refused() {
    assert_eq!(
        kind(decode::<u32>(&[1, 0, 0, 0, 0])),
        ErrorKind::Trailing(1)
    );
    assert_eq!(kind(decode::<u32>(&[1, 0])), ErrorKind::Truncated);
    assert_eq!(kind(decode::<bool>(&[2])), ErrorKind::OutOfRange);
    assert_eq!(
        kind(decode::<char>(&0xd800u32.to_le_bytes())),
        ErrorKind::OutOfRange
    );
    assert_eq!(kind(decode::<String>(&[0xff])), ErrorKind::Utf8);
    // PersonV1's name (field 1) as a byte, not a string.
    assert_eq!(
        kind(decode::<PersonV1>(&[0x41, 1])),
        ErrorKind::Wire {
            expected: Wire::Bytes,
            found: Wire::Fixed8
        }
    );
    // Its age (field 2, four bytes) twice.
    assert_eq!(
        kind(decode::<PersonV1>(&[
            0x83, 0x01, 1, 0, 0, 0, 0x83, 0x01, 2, 0, 0, 0
        ])),
        ErrorKind::Repeated(2)
    );
    // A name claiming five bytes, with one there.
    assert_eq!(
        kind(decode::<PersonV1>(&[0x47, 5, 0, 0, 0, b'a'])),
        ErrorKind::Truncated
    );
    // A spare wire type, a key that never ends, and id zero.
    assert_eq!(kind(decode::<PersonV1>(&[0x4b])), ErrorKind::BadKey);
    assert_eq!(kind(decode::<PersonV1>(&[0xff; 7])), ErrorKind::BadKey);
    assert_eq!(kind(decode::<PersonV1>(&[0x08])), ErrorKind::BadKey);
    assert_eq!(
        kind(decode::<[u8; 2]>(&bytes(&[1u8, 2, 3]))),
        ErrorKind::OutOfRange
    );
    assert_eq!(
        kind(decode::<Vec<u16>>(&[Number::U16 as u8, 1, 2, 3])),
        ErrorKind::Truncated
    );
    let mut w = Writer::new();
    w.list_header(Wire::Message, 2).unwrap();
    for _ in 0..2 {
        w.length(Wire::Message, |w| w.field(Key::new(1), "k"))
            .unwrap();
    }
    assert_eq!(
        kind(decode::<BTreeMap<String, u32>>(&w.into_bytes())),
        ErrorKind::RepeatedKey
    );
}
