use super::*;

#[test]
fn a_missing_field_with_no_default_is_an_error_naming_where() {
    let error = decode::<Holder>(&[]).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Missing);
    assert_eq!(error.path().collect::<Vec<_>>(), ["Holder.shape"]);
    // A holder whose own holder has no shape: the path names both.
    let mut w = Writer::new();
    w.message(Key::new(1), |_| Ok(())).unwrap();
    let error = decode::<HolderOfHolder>(&w.into_bytes()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "HolderOfHolder.holder > Holder.shape: Missing"
    );
    let holder = Holder {
        shape: ClosedShape::Dot,
    };
    assert_eq!(decode::<Holder>(&bytes(&holder)), Ok(holder));
}
