use super::*;
use be_block_protocol::PushService;

fn pushing_to(hosts: &[&str]) -> ServerConfig {
    ServerConfig {
        push: push::PushConfig {
            allow_local_endpoints: true,
            fcm: None,
            hosts: hosts.iter().map(|host| (*host).to_owned()).collect(),
        },
        ..ServerConfig::OPEN
    }
}

#[tokio::test]
async fn an_endpoint_on_a_host_taken_off_the_list_is_dropped_at_send() {
    let mut harness = Harness::configured(pushing_to(&["127.0.0.1", "127.0.0.2"])).await;
    let mut service = TestPushService::start(|_| (201, String::new())).await;
    let port = service.url.rsplit(':').next().unwrap().to_owned();
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.personal().await;
    let block = owner.create_block(BlockParent::Root).await;
    let (mut member, account) = viewer_of(&harness, &mut owner, block, "member@example.com").await;
    member
        .register_endpoint(
            TestDevice::new()
                .endpoint(PushService::WebPush, format!("http://127.0.0.2:{port}/old")),
        )
        .await;
    member
        .register_endpoint(
            TestDevice::new().endpoint(PushService::WebPush, format!("{}/kept", service.url)),
        )
        .await;
    member.sign_out().await;
    let token = owner.token.clone();
    drop(owner);

    // The operator takes 127.0.0.2 off the list.
    harness.server.restart_with(pushing_to(&["127.0.0.1"])).await;
    let mut owner = harness.second_connection(&token, organization).await;
    owner.notify(block, b"sealed", Vec::new()).await;
    // One account's pushes go in the order its devices registered, so the old endpoint was
    // judged before the kept one heard anything.
    assert_eq!(service.next().await.path, "/kept");
    let kept = ServerStore::open(&harness.directory)
        .unwrap()
        .push_endpoints(account, organization)
        .await
        .unwrap();
    assert_eq!(
        kept.iter().map(|endpoint| endpoint.address.as_str()).collect::<Vec<_>>(),
        [format!("{}/kept", service.url)],
        "the endpoint on a host no longer listed was dropped without a request"
    );

    harness.stop().await;
}
