use super::*;

#[test]
fn a_value_with_bytes_after_it_is_not_decoded() {
    let mut bytes = Vec::new();
    ciborium::into_writer(&(7u32, "seven"), &mut bytes).unwrap();
    assert_eq!(decode::<(u32, String)>(&bytes), Ok((7, "seven".to_owned())));

    let mut longer = bytes.clone();
    longer.push(0);
    assert_eq!(decode::<(u32, String)>(&longer), Err(Error::Trailing(1)));
    assert!(matches!(decode::<(u32, String)>(&[]), Err(Error::Value(_))));
}
