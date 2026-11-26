use super::*;

#[test]
fn malformed_input_is_refused() {
    assert_eq!(decode::<u32>(&[1, 0]), Err(Error::Trailing(1)));
    assert_eq!(decode::<u8>(&[0x80, 0x02]), Err(Error::OutOfRange));
    assert_eq!(decode::<bool>(&[2]), Err(Error::OutOfRange));
    assert_eq!(decode::<String>(&[0xff]), Err(Error::Utf8));
    assert_eq!(decode::<u64>(&[0xff; 11]), Err(Error::Overlong));
    assert_eq!(
        decode::<PersonV1>(&[0x08, 1]),
        Err(Error::Wire {
            expected: Wire::Len,
            found: Wire::Varint
        })
    );
    assert_eq!(
        decode::<PersonV1>(&[0x10, 1, 0x10, 2]),
        Err(Error::Repeated(2))
    );
    assert_eq!(decode::<PersonV1>(&[0x0a, 5, b'a']), Err(Error::Truncated));
    assert_eq!(decode::<PersonV1>(&[0x0b]), Err(Error::BadKey(0x0b)));
    assert_eq!(
        decode::<[u8; 2]>(&encode(&[1u8, 2, 3])),
        Err(Error::OutOfRange)
    );
    let mut repeated_key = Vec::new();
    for _ in 0..2 {
        with_length(&mut repeated_key, |out| write_field(1, "k", out));
    }
    assert_eq!(
        decode::<BTreeMap<String, u32>>(&repeated_key),
        Err(Error::RepeatedKey)
    );
}
