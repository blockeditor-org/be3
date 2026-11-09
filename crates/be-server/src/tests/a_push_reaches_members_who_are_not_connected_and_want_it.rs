use super::*;
use be_block_protocol::{NotifyMode, PushMessage, PushService};
use ring::signature::{ECDSA_P256_SHA256_FIXED, UnparsedPublicKey};

#[tokio::test]
async fn a_push_reaches_members_who_are_not_connected_and_want_it() {
    let harness = Harness::configured(pushing_locally()).await;
    let mut service = TestPushService::start(|_| (201, String::new())).await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    let organization = owner.personal().await;
    let block = owner.create_block(BlockParent::Root).await;
    let key = owner
        .call(|peer| client::notifications::push_key(peer))
        .await
        .expect("a signed-in client reads the push key");
    let offers = owner
        .call(|peer| client::notifications::offers_fcm(peer))
        .await;
    assert!(matches!(offers, Ok(false)), "{offers:?}");

    let (mut away, _) = viewer_of(&harness, &mut owner, block, "away@example.com").await;
    let away_device = TestDevice::new();
    away.register_endpoint(
        away_device.endpoint(PushService::WebPush, format!("{}/away", service.url)),
    )
    .await;
    away.sign_out().await;
    let (mut present, _) = viewer_of(&harness, &mut owner, block, "present@example.com").await;
    let present_device = TestDevice::new();
    present
        .register_endpoint(
            present_device.endpoint(PushService::WebPush, format!("{}/present", service.url)),
        )
        .await;
    let (mut quiet, quiet_id) = viewer_of(&harness, &mut owner, block, "quiet@example.com").await;
    let quiet_device = TestDevice::new();
    quiet
        .register_endpoint(
            quiet_device.endpoint(PushService::WebPush, format!("{}/quiet", service.url)),
        )
        .await;
    quiet.set_notify_mode(block, NotifyMode::Mentions).await;
    quiet.sign_out().await;

    owner.notify(block, b"first", Vec::new()).await;
    let pushed = service.next().await;
    assert_eq!(pushed.path, "/away");
    assert_eq!(pushed.headers["content-encoding"], "aes128gcm");
    assert!(pushed.headers.contains_key("ttl"), "{:?}", pushed.headers);
    let authorization = &pushed.headers["authorization"];
    let (token, public) = authorization
        .strip_prefix("vapid t=")
        .and_then(|rest| rest.split_once(", k="))
        .expect("the push carries a VAPID authorization");
    use base64::Engine;
    let base64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    assert_eq!(base64.decode(public).unwrap(), key);
    let (signed, signature) = token.rsplit_once('.').unwrap();
    UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, &key)
        .verify(signed.as_bytes(), &base64.decode(signature).unwrap())
        .expect("the server signed the token with its push key");
    let claims = base64.decode(signed.split('.').nth(1).unwrap()).unwrap();
    let claims: ureq::serde_json::Value = ureq::serde_json::from_slice(&claims).unwrap();
    assert_eq!(claims["aud"], service.url.as_str());
    assert_eq!(
        away_device.open(&pushed.body),
        PushMessage {
            organization,
            block,
            sealed: b"first".to_vec(),
        }
    );

    // Each account's pushes go in order, so the first that reaches the present member, once
    // gone, and the quiet one, once mentioned, would be the earlier notification had it
    // reached them.
    present.sign_out().await;
    owner.notify(block, b"second", vec![quiet_id]).await;
    let mut seen = Vec::new();
    let (mut present_device, mut quiet_device) = (Some(present_device), Some(quiet_device));
    while seen.len() < 3 {
        let pushed = service.next().await;
        let device = match pushed.path.as_str() {
            "/present" => present_device.take(),
            "/quiet" => quiet_device.take(),
            "/away" => None,
            other => panic!("a push went to {other}"),
        };
        if let Some(device) = device {
            assert_eq!(device.open(&pushed.body).sealed, b"second");
        }
        seen.push(pushed.path);
    }
    seen.sort();
    assert_eq!(seen, ["/away", "/present", "/quiet"]);

    harness.stop().await;
}
