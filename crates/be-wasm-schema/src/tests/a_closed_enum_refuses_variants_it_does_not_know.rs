use super::*;

#[test]
fn a_closed_enum_refuses_variants_it_does_not_know() {
    let bytes = encode(&ShapeV2::Square {
        side: 2.0,
        label: String::new(),
    });
    assert_eq!(decode::<ClosedShape>(&bytes), Err(Error::UnknownVariant(3)));
    assert_eq!(decode::<ClosedShape>(&[]), Err(Error::NotOneVariant));
}
