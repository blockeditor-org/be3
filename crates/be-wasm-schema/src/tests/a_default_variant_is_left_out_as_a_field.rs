use super::*;

#[test]
fn a_default_variant_is_left_out_as_a_field() {
    let dot = Drawing {
        shape: ShapeV2::Dot,
    };
    assert_eq!(encode(&dot), Vec::<u8>::new());
    assert_eq!(decode::<Drawing>(&[]), Ok(dot));
}
