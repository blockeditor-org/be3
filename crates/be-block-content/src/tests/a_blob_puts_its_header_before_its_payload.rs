use super::*;

#[test]
fn a_blob_puts_its_header_before_its_payload() {
    let payload: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
    let content = image("frame.png", &payload);
    let encoded = content.encode();

    let start = content.payload_offset() as usize;
    assert!(start < 64, "the header took {start} bytes");
    assert_eq!(&encoded[start..], payload.as_slice());

    let header_only = BytesContent::decode(&encoded[..start]).unwrap();
    assert_eq!(header_only.header(), &header("frame.png"));
    assert!(header_only.data().is_empty());
    assert_eq!(header_only.payload_offset() as usize, start);

    assert_eq!(BytesContent::decode(&encoded).unwrap(), content);
    assert_eq!(
        BytesContent::decode(&encoded[..start - 1]),
        Err(ContentError::Truncated)
    );
    assert_eq!(BytesContent::decode(&[1, 2]), Err(ContentError::Truncated));
    assert!(BytesContent::decode(&[0xff, 0xff, 0, 0, 7]).is_err());
}
