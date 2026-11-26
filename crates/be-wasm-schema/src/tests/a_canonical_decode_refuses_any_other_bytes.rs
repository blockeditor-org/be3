use super::*;

#[test]
fn a_canonical_decode_refuses_any_other_bytes() {
    let person = PersonV1 {
        name: "Ada".into(),
        age: 36,
    };
    let encoded = bytes(&person);
    assert_eq!(decode_canonical::<PersonV1>(&encoded), Ok(person));
    // The same fields in the other order decode, but are not canonical.
    let mut w = Writer::new();
    w.field(Key::new(2), &36u32).unwrap();
    w.field(Key::new(1), "Ada").unwrap();
    let reordered = w.into_bytes();
    assert!(decode::<PersonV1>(&reordered).is_ok());
    assert_eq!(
        kind(decode_canonical::<PersonV1>(&reordered)),
        ErrorKind::NotCanonical
    );
    // As is a field holding its default.
    let mut w = Writer::new();
    w.field(Key::new(1), "").unwrap();
    assert_eq!(
        kind(decode_canonical::<PersonV1>(&w.into_bytes())),
        ErrorKind::NotCanonical
    );
}
