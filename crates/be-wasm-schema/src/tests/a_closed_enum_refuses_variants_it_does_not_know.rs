use super::*;

#[test]
fn a_closed_enum_refuses_variants_it_does_not_know() {
    let bytes = bytes(&ShapeV2::Square {
        side: 2.0,
        label: String::new(),
    });
    assert_eq!(
        kind(decode::<ClosedShape>(&bytes)),
        ErrorKind::UnknownVariant(3)
    );
    assert_eq!(kind(decode::<ClosedShape>(&[])), ErrorKind::NotOneVariant);
}
