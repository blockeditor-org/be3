use super::*;

#[test]
fn a_screenshot_is_saved_and_its_uri_answered() {
    let bus = PrivateBus::start();
    let directory = std::env::temp_dir().join(format!("be3-shots-{}", uuid::Uuid::new_v4()));
    let pixels = vec![255, 0, 0, 255, 0, 0, 255, 128];
    serving(
        &bus,
        Some(Image::from_rgba(2, 1, pixels.clone())),
        directory.clone(),
    );

    let (response, results) = zbus::block_on(async {
        let client = bus.connect().await;
        owned(&client, NAME).await;
        screenshot(&client).await
    });

    assert_eq!(response, SUCCESS);
    let uri = String::try_from(results["uri"].try_clone().expect("a uri")).expect("a string");
    let path = uri.strip_prefix("file://").expect("a file URI");
    assert!(
        Path::new(path).starts_with(&directory),
        "{path} is in the session's directory"
    );
    let saved = image::open(path).expect("a PNG").into_rgba8();
    assert_eq!(saved.dimensions(), (2, 1));
    assert_eq!(saved.into_raw(), pixels);
    let _ = std::fs::remove_dir_all(directory);
}
