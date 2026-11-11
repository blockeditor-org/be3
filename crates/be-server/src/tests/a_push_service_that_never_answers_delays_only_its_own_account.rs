use super::*;
use be_block_protocol::PushService;

#[tokio::test]
async fn a_push_service_that_never_answers_delays_only_its_own_account() {
    let harness = Harness::configured(pushing_locally()).await;
    let mut service = TestPushService::start(|path| match path {
        "/tarpit" => (0, String::new()),
        _ => (201, String::new()),
    })
    .await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let block = owner.create_block(BlockParent::Root).await;
    let (mut slow, _) = viewer_of(&harness, &mut owner, block, "slow@example.com").await;
    slow.register_endpoint(
        TestDevice::new().endpoint(PushService::WebPush, format!("{}/tarpit", service.url)),
    )
    .await;
    slow.sign_out().await;
    let (mut quick, _) = viewer_of(&harness, &mut owner, block, "quick@example.com").await;
    quick
        .register_endpoint(
            TestDevice::new().endpoint(PushService::WebPush, format!("{}/quick", service.url)),
        )
        .await;
    quick.sign_out().await;

    owner.notify(block, b"first", Vec::new()).await;
    let mut paths = vec![service.next().await.path, service.next().await.path];
    paths.sort();
    assert_eq!(
        paths,
        ["/quick", "/tarpit"],
        "the quick device hears while the tarpit holds its push"
    );
    owner.notify(block, b"second", Vec::new()).await;
    assert_eq!(
        service.next().await.path,
        "/quick",
        "the next push reaches the quick device though the tarpit still holds the first"
    );

    harness.stop().await;
}
