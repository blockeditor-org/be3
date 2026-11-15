#[test]
fn a_stored_value_with_bytes_after_it_is_not_read() {
    let bytes = crate::to_cbor(&(3u64, "three"));
    assert_eq!(
        crate::from_cbor::<(u64, String)>(&bytes),
        Some((3, "three".to_owned()))
    );
    let mut longer = bytes;
    longer.push(0);
    assert_eq!(crate::from_cbor::<(u64, String)>(&longer), None);
}
