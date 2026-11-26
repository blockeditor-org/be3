use super::*;

#[test]
fn a_stored_value_with_bytes_after_it_is_not_decoded() {
    let bytes = manifest::encode_value(&(3u64, "three")).unwrap();
    assert_eq!(
        decode_value::<(u64, String)>(&bytes).unwrap(),
        (3, "three".to_owned())
    );
    let mut longer = bytes;
    longer.push(0);
    assert!(matches!(
        decode_value::<(u64, String)>(&longer),
        Err(StoreError::Encoding)
    ));
}
