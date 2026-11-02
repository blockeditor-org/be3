use super::*;

#[test]
fn an_image_is_decoded_into_its_header_and_keeps_its_bytes() {
    let mut image = ImageFile::from_file("photo.png", vec![1, 2, 3]);
    let mut decoded = image.header().clone();
    decoded.width = 4;
    decoded.height = 3;
    image.apply(&ImageOp::SetHeader(decoded));
    assert_eq!(image.header().size(), Some((4, 3)));
    assert_eq!(image.data(), [1, 2, 3]);
    assert_eq!(ImageContent::decode(&image.encode()).unwrap(), image);
}
