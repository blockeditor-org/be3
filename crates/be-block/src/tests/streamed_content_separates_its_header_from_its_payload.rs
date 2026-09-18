use super::*;

#[test]
fn streamed_content_separates_its_header_from_its_payload() {
    let payload: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
    let content = image("frame.png", &payload);
    let encoded = content.encode();

    let start = payload_start(&encoded).unwrap();
    assert!(start < 64, "the header took {start} bytes");
    assert_eq!(&encoded[start..], payload.as_slice());
    assert_eq!(
        payload_start(&encoded[..HEADER_PREFIX_BYTES]).unwrap(),
        start
    );

    let (decoded_header, decoded_payload) =
        decode_streamed::<ImageHeader>(&encoded[..start + 16]).unwrap();
    assert_eq!(decoded_header, header("frame.png"));
    assert_eq!(decoded_payload, payload[..16]);

    assert_eq!(ImageContent::decode(&encoded).unwrap(), content);
    assert_eq!(content.payload(), payload.as_slice());
    assert_eq!(
        ImageContent::from_parts(header("frame.png"), payload.clone()),
        content
    );

    assert_eq!(payload_start(&[1, 2]), Err(ContentError::Truncated));
    assert_eq!(
        ImageContent::decode(&encoded[..start - 1]),
        Err(ContentError::Truncated)
    );
    assert!(ImageContent::decode(&[0xff, 0xff, 0, 0, 7]).is_err());
}
