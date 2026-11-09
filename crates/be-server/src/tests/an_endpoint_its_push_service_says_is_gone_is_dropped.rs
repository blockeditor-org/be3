use super::*;
use be_block_protocol::PushService;

#[tokio::test]
async fn an_endpoint_its_push_service_says_is_gone_is_dropped() {
    let harness = Harness::configured(pushing_locally()).await;
    let mut service = TestPushService::start(|path| match path {
        "/gone" => (410, String::new()),
        _ => (201, String::new()),
    })
    .await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.personal().await;
    let block = owner.create_block(BlockParent::Root).await;
    let (mut member, account) = viewer_of(&harness, &mut owner, block, "member@example.com").await;
    member
        .register_endpoint(
            TestDevice::new().endpoint(PushService::WebPush, format!("{}/gone", service.url)),
        )
        .await;
    member
        .register_endpoint(
            TestDevice::new().endpoint(PushService::WebPush, format!("{}/kept", service.url)),
        )
        .await;
    member.sign_out().await;

    // One account's pushes go one at a time in the order its devices registered, so the gone
    // endpoint is dropped before the kept one hears the first notification.
    owner.notify(block, b"first", Vec::new()).await;
    assert_eq!(service.next().await.path, "/gone");
    assert_eq!(service.next().await.path, "/kept");
    let store = ServerStore::open(&harness.directory).unwrap();
    let kept = store.push_endpoints(account, organization).await.unwrap();
    assert_eq!(kept.len(), 1);
    assert!(kept[0].address.ends_with("/kept"), "{kept:?}");
    owner.notify(block, b"second", Vec::new()).await;
    assert_eq!(service.next().await.path, "/kept");

    harness.stop().await;
}
