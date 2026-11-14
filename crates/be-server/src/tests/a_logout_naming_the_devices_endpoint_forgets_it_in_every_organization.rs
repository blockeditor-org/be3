use super::*;
use be_block_protocol::PushService;

#[tokio::test]
async fn a_logout_naming_the_devices_endpoint_forgets_it_in_every_organization() {
    let harness = Harness::configured(pushing_locally()).await;
    let mut device = harness.client().await;
    let account = device.register("device@example.com").await;
    let address = "http://127.0.0.1:9/device".to_owned();
    let endpoint = || TestDevice::new().endpoint(PushService::WebPush, address.clone());
    let personal = device.personal().await;
    device.register_endpoint(endpoint()).await;
    device
        .register_endpoint(
            TestDevice::new().endpoint(PushService::WebPush, "http://127.0.0.1:9/other".into()),
        )
        .await;
    let team = device.organization("Team").await;
    device.register_endpoint(endpoint()).await;

    let logged_out = device
        .call(|peer| client::account::logout(peer, Some(address.clone())))
        .await;
    assert!(logged_out.is_ok(), "{logged_out:?}");

    let store = ServerStore::open(&harness.directory).unwrap();
    let kept = store.push_endpoints(account, personal).await.unwrap();
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(
        kept[0].address.ends_with("/other"),
        "another device's endpoint is kept: {kept:?}"
    );
    assert!(store.push_endpoints(account, team).await.unwrap().is_empty());

    harness.stop().await;
}
