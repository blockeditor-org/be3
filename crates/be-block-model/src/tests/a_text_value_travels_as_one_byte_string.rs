#[test]
fn a_text_value_travels_as_one_byte_string() {
    let value = crate::Value::Text(crate::Sequence::from_items(b"hello".to_vec()));
    let sent = postcard::to_stdvec(&value).expect("a value encodes");

    let mut byte_by_byte = postcard::to_stdvec(&6u32).expect("a variant encodes");
    byte_by_byte.extend(postcard::to_stdvec(&b"hello".to_vec()).expect("bytes encode"));
    assert_eq!(
        sent, byte_by_byte,
        "postcard writes it as it wrote each byte"
    );
    assert_eq!(
        postcard::from_bytes::<crate::Value>(&sent).expect("it decodes"),
        value
    );
}
