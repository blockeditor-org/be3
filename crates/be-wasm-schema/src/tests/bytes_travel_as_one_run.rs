use super::*;

#[test]
fn bytes_travel_as_one_run() {
    let data: Vec<u8> = (0..1000).map(|i| (i % 251) as u8).collect();
    let encoded = bytes(&Blob { data: data.clone() });
    // A key, a four-byte length and the bytes as they are.
    assert_eq!(encoded.len(), 1 + 4 + 1000);
    assert_eq!(&encoded[5..], &data[..]);
    assert_eq!(decode::<Blob>(&encoded), Ok(Blob { data }));
}
