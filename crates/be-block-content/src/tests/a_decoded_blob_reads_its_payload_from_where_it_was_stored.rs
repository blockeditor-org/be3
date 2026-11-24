use super::*;

/// A blob decoded from its stored form reports where its payload starts there, even when its
/// header would encode to another length now (here two bytes the stored header carries past what
/// decoding reads), and goes back to encoding the header once the header changes.
#[test]
fn a_decoded_blob_reads_its_payload_from_where_it_was_stored() {
    let payload = b"pixels".to_vec();
    let encoded = image("frame.png", &payload).encode();
    let start = image("frame.png", &payload).payload_offset() as usize;
    let mut stored = encoded[..start].to_vec();
    let length = u32::from_le_bytes(stored[..4].try_into().unwrap()) + 2;
    stored[..4].copy_from_slice(&length.to_le_bytes());
    stored.extend_from_slice(&[0, 0]);
    stored.extend_from_slice(&payload);

    let mut decoded = BytesContent::decode(&stored).unwrap();
    assert_eq!(decoded.payload_offset() as usize, start + 2);
    assert_eq!(decoded.data(), payload.as_slice());

    decoded.set_header(header("other.png"));
    assert_eq!(
        decoded.payload_offset() as usize,
        BytesContent::new(header("other.png"), Vec::new()).encode().len()
    );
}
