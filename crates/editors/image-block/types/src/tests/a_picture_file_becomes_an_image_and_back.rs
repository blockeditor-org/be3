use super::*;

#[test]
fn a_picture_file_becomes_an_image_and_back() {
    let claims = |name: &str| {
        ask(Request::Claims {
            name: name.into(),
            first: vec![0x89, b'P', 0],
            directory: false,
        })
    };
    assert_eq!(
        claims("logo.png"),
        Reply::Claimed(Some(ImageContent::CONTENT_TYPE))
    );
    assert_eq!(claims("logo.paint"), Reply::Claimed(None));

    let Reply::Content(bytes) = ask(Request::Apply {
        content_type: ImageContent::CONTENT_TYPE,
        bytes: None,
        client: 3,
        change: Change::FromFile {
            name: "logo.png".into(),
            bytes: vec![1, 2, 3],
        },
    }) else {
        panic!("a picture becomes an image");
    };
    let image = ImageContent::decode(&bytes).unwrap();
    assert_eq!(image.header().source_name, "logo.png");
    assert_eq!(image.header().media_type, "image/png");
    assert_eq!(
        ask(Request::ToFile {
            content_type: ImageContent::CONTENT_TYPE,
            bytes,
        }),
        Reply::File(Some(vec![1, 2, 3]))
    );
}
