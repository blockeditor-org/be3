use super::*;

#[test]
fn a_missing_field_with_no_default_is_an_error() {
    assert_eq!(decode::<Holder>(&[]), Err(Error::Missing("Holder.shape")));
    assert_eq!(
        decode::<Holder>(&encode(&Holder {
            shape: ClosedShape::Dot
        })),
        Ok(Holder {
            shape: ClosedShape::Dot
        })
    );
}
