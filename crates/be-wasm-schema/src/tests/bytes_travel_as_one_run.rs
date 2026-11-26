use super::*;

#[test]
fn bytes_travel_as_one_run() {
    let data: Vec<u8> = (0..1000).map(|i| (i % 251) as u8).collect();
    let bytes = encode(&Blob { data: data.clone() });
    // A key, a two-byte length and the bytes as they are.
    assert_eq!(bytes.len(), 1 + 2 + 1000);
    assert_eq!(&bytes[3..], &data[..]);
    assert_eq!(decode::<Blob>(&bytes), Ok(Blob { data }));
}
