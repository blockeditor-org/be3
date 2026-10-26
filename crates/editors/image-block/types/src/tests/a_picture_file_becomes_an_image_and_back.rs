use super::*;

#[test]
fn a_picture_file_becomes_an_image_and_back() {
    let module = crate::block_types();
    let claims = |name: &str| module.claims(name.into(), vec![0x89, b'P', 0], false, false);
    assert_eq!(claims("logo.png"), Some(ImageContent::CONTENT_TYPE));
    assert_eq!(claims("logo.paint"), None);

    let Ok(Some(bytes)) = module.apply(
        ImageContent::CONTENT_TYPE,
        None,
        3,
        Change::FromFile(FileChange {
            name: "logo.png".into(),
            bytes: vec![1, 2, 3],
        }),
    ) else {
        panic!("a picture becomes an image");
    };
    let image = ImageContent::decode(&bytes).unwrap();
    assert_eq!(image.header().source_name, "logo.png");
    assert_eq!(image.header().media_type, "image/png");
    assert_eq!(
        module.to_file(ImageContent::CONTENT_TYPE, bytes),
        Ok(Some(PlainFile {
            bytes: vec![1, 2, 3],
            name: Some("logo.png".to_owned()),
            extension: "png".to_owned(),
            media_type: "image/png".to_owned(),
        }))
    );
}
