use super::*;

#[test]
fn no_picture_fails_the_request() {
    let bus = PrivateBus::start();
    let directory = std::env::temp_dir().join(format!("be3-shots-{}", uuid::Uuid::new_v4()));
    serving(&bus, None, directory.clone());

    let (response, results) = zbus::block_on(async {
        let client = bus.connect().await;
        owned(&client, NAME).await;
        screenshot(&client).await
    });

    assert_eq!(response, FAILED);
    assert!(results.is_empty());
    assert!(!directory.exists(), "nothing was saved");
}
