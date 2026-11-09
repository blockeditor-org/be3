use super::*;
use be_block_protocol::PushService;

#[tokio::test]
async fn a_device_registers_only_an_endpoint_the_server_would_push_to() {
    let harness = Harness::start().await;
    let mut member = harness.client().await;
    member.register("member@example.com").await;
    member.personal().await;
    let device = TestDevice::new();
    let mut refused = Vec::new();
    for (endpoint, why) in [
        (
            device.endpoint(PushService::WebPush, "http://push.example.com/a".into()),
            "not https",
        ),
        (
            device.endpoint(PushService::WebPush, "not a url".into()),
            "not a URL",
        ),
        (
            device.endpoint(PushService::Fcm, "a-registration-token".into()),
            "FCM on a server without it",
        ),
        (
            be_block_protocol::PushEndpoint {
                p256dh: vec![4; 33],
                ..device.endpoint(PushService::WebPush, "https://push.example.com/a".into())
            },
            "a compressed key",
        ),
    ] {
        let registered = member
            .call(|peer| client::notifications::register_endpoint(peer, endpoint))
            .await;
        if !refused_as(&registered, ErrorCode::InvalidRequest) {
            refused.push((why, registered));
        }
    }
    assert!(refused.is_empty(), "registered {refused:?}");
    member
        .register_endpoint(device.endpoint(PushService::WebPush, "https://push.example.com/a".into()))
        .await;

    for (address, public) in [
        ("127.0.0.1", false),
        ("10.1.2.3", false),
        ("192.168.0.1", false),
        ("169.254.169.254", false),
        ("100.64.0.1", false),
        ("::1", false),
        ("fd00::1", false),
        ("fe80::1", false),
        ("::ffff:10.0.0.1", false),
        ("8.8.8.8", true),
        ("2606:4700::1111", true),
    ] {
        assert_eq!(
            push::is_public(address.parse().unwrap()),
            public,
            "{address}"
        );
    }

    harness.stop().await;
}
