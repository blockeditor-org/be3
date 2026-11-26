use super::*;

#[test]
fn malformed_input_is_refused() {
    assert_eq!(decode::<u32>(&[1, 0, 0, 0, 0]), Err(Error::Trailing(1)));
    assert_eq!(decode::<u32>(&[1, 0]), Err(Error::Truncated));
    assert_eq!(decode::<bool>(&[2]), Err(Error::OutOfRange));
    assert_eq!(
        decode::<char>(&0xd800u32.to_le_bytes()),
        Err(Error::OutOfRange)
    );
    assert_eq!(decode::<String>(&[0xff]), Err(Error::Utf8));
    // PersonV1's name (field 1) as a byte, not a string.
    assert_eq!(
        decode::<PersonV1>(&[0x08, 1]),
        Err(Error::Wire {
            expected: Wire::Len,
            found: Wire::Fixed8
        })
    );
    // Its age (field 2, four bytes) twice.
    assert_eq!(
        decode::<PersonV1>(&[0x12, 1, 0, 0, 0, 0x12, 2, 0, 0, 0]),
        Err(Error::Repeated(2))
    );
    // A name claiming five bytes, with one there.
    assert_eq!(
        decode::<PersonV1>(&[0x0c, 5, 0, 0, 0, b'a']),
        Err(Error::Truncated)
    );
    // Wire type 5, a key that never ends, and id zero.
    assert_eq!(decode::<PersonV1>(&[0x0d]), Err(Error::BadKey));
    assert_eq!(decode::<PersonV1>(&[0xff; 6]), Err(Error::BadKey));
    assert_eq!(decode::<PersonV1>(&[0x04, 0, 0, 0, 0]), Err(Error::BadKey));
    assert_eq!(
        decode::<[u8; 2]>(&encode(&[1u8, 2, 3])),
        Err(Error::OutOfRange)
    );
    assert_eq!(decode::<Vec<u16>>(&[1, 2, 3]), Err(Error::Truncated));
    let mut repeated_key = Vec::new();
    for _ in 0..2 {
        with_length(&mut repeated_key, |out| write_field(1, "k", out));
    }
    assert_eq!(
        decode::<BTreeMap<String, u32>>(&repeated_key),
        Err(Error::RepeatedKey)
    );
}
