use super::*;

#[test]
fn an_open_enum_keeps_variants_it_does_not_know() {
    let square = ShapeV2::Square {
        side: 2.0,
        label: "box".into(),
    };
    let bytes = encode(&square);
    let kept: ShapeV1 = decode(&bytes).unwrap();
    assert!(matches!(&kept, ShapeV1::Other(variant) if variant.id == 3));
    assert_eq!(encode(&kept), bytes);
    assert_eq!(decode::<ShapeV2>(&encode(&kept)), Ok(square));
    assert_eq!(
        decode::<ShapeV1>(&encode(&ShapeV2::Circle(1.5))),
        Ok(ShapeV1::Circle(1.5))
    );
}
