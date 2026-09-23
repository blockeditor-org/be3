use super::{decode, png_bytes};

#[test]
fn decode_reads_the_pixels_and_the_metadata() {
    let decoded = decode(&png_bytes()).unwrap();

    assert_eq!(decoded.media_type, "image/png");
    assert_eq!((decoded.width, decoded.height), (2, 1));
    assert_eq!(decoded.pixels, vec![255, 0, 0, 255, 0, 255, 0, 128]);
    assert!(decode(b"not an image").is_err());
}
