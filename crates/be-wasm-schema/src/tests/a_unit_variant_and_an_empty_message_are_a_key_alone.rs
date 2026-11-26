use super::*;

#[test]
fn a_unit_variant_and_an_empty_message_are_a_key_alone() {
    // A unit variant is its key: (2 << 6) | Empty, two bytes of varint.
    assert_eq!(bytes(&ModeV2::On), [0x80, 0x01]);
    // A message whose fields are all left out is written as nothing: field 1, Empty.
    let empty = Wrapper {
        inner: PersonV1 {
            name: String::new(),
            age: 0,
        },
    };
    assert_eq!(bytes(&empty), [0x40]);
    assert_eq!(decode::<Wrapper>(&[0x40]), Ok(empty));
    // And a default variant as a field is left out entirely.
    let dot = Drawing {
        shape: ShapeV2::Dot,
    };
    assert_eq!(bytes(&dot), Vec::<u8>::new());
    assert_eq!(decode::<Drawing>(&[]), Ok(dot));
}
