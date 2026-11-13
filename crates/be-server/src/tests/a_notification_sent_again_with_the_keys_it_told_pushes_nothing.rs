use super::*;
use be_block_protocol::PushService;

/// The next push to each of the member's three devices, opened with the device at `opened`:
/// a test device opens one push.
async fn heard(service: &mut TestPushService, opened: &str, device: TestDevice) -> Vec<u8> {
    let mut device = Some(device);
    let mut sealed = None;
    for path in ["/1", "/2", "/3"] {
        let pushed = service.next().await;
        assert_eq!(pushed.path, path, "pushes go to the devices in order");
        if path == opened
            && let Some(device) = device.take()
        {
            sealed = Some(device.open(&pushed.body).sealed);
        }
    }
    sealed.expect("the device was pushed to")
}

#[tokio::test]
async fn a_notification_sent_again_with_the_keys_it_told_pushes_nothing() {
    let harness = Harness::configured(pushing_locally()).await;
    let mut service = TestPushService::start(|_| (201, String::new())).await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let block = owner.create_block(BlockParent::Root).await;
    let (mut member, _) = viewer_of(&harness, &mut owner, block, "member@example.com").await;
    let devices = [TestDevice::new(), TestDevice::new(), TestDevice::new()];
    for (at, device) in devices.iter().enumerate() {
        member
            .register_endpoint(
                device.endpoint(PushService::WebPush, format!("{}/{}", service.url, at + 1)),
            )
            .await;
    }
    member.sign_out().await;
    let [first, second, third] = devices;

    owner.tell(block, b"first", &["one"]).await;
    assert_eq!(heard(&mut service, "/1", first).await, b"first");
    // A device that never heard the answer sends the same message again, alone or with one
    // already told; one more key is news.
    owner.tell(block, b"first again", &["one"]).await;
    owner.tell(block, b"second", &["one", "two"]).await;
    assert_eq!(
        heard(&mut service, "/2", second).await,
        b"second",
        "what was told before is not pushed again"
    );
    owner.tell(block, b"second again", &["two", "one"]).await;
    owner.tell(block, b"third", &["three"]).await;
    assert_eq!(heard(&mut service, "/3", third).await, b"third");

    harness.stop().await;
}
