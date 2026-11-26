use super::*;

/// A blob decoded from only the start of its stored form (its header, the payload left in the
/// store) answers where its payload is: right after the header, as long as the rest of the
/// stored form. A header edited since, which encodes longer (an image recording its size), moves
/// where the payload starts as its replica's content has it, and keeps its length: the length of
/// the stored form less where the header now ends would cut the payload short.
#[test]
fn a_blob_decoded_without_its_payload_keeps_its_payloads_range_as_its_header_grows() {
    let payload = b"pixels of a picture".to_vec();
    let stored = image("frame.png", &payload).encode();
    let start = image("frame.png", &payload).payload_offset();
    let length = stored.len() as u64;

    let mut decoded = BytesContent::decode(&stored[..start as usize]).unwrap();
    assert_eq!(decoded.payload_range(length), (start, payload.len() as u64));

    let longer = header("a much longer name for the frame.png");
    let moved = BytesContent::new(longer.clone(), Vec::new()).encode().len() as u64;
    assert!(moved > start);
    decoded.set_header(longer);
    assert_eq!(decoded.payload_range(length), (moved, payload.len() as u64));
}
