use super::*;
use be_block_protocol::PushService;

#[tokio::test]
async fn a_device_registers_only_an_endpoint_the_server_would_push_to() {
    let harness = Harness::configured(ServerConfig {
        push: push::PushConfig {
            hosts: vec!["ntfy.example.org".to_owned()].into(),
            ..push::PushConfig::DEFAULT
        },
        ..ServerConfig::OPEN
    })
    .await;
    let mut member = harness.client().await;
    member.register("member@example.com").await;
    member.personal().await;
    let device = TestDevice::new();
    let web = |address: &str| device.endpoint(PushService::WebPush, address.to_owned());
    let mut wrong = Vec::new();
    for (endpoint, why) in [
        (web("http://fcm.googleapis.com/fcm/send/a"), "not https"),
        (web("not a url"), "not a URL"),
        (
            web("https://fcm.googleapis.com:25/fcm/send/a"),
            "a port other than 443",
        ),
        (
            web("https://push.example.com/a"),
            "a host no push service has",
        ),
        (
            web("https://fcm.googleapis.com.example.com/a"),
            "a push service's host inside another",
        ),
        (web("https://notify.windows.com/a"), "a suffix's own domain"),
        (
            web("https://evilnotify.windows.com/a"),
            "a host ending in a suffix without its dot",
        ),
        (
            web("https://ntfy.example.org.evil.example/a"),
            "an added host inside another",
        ),
        (
            web("https://evil.example@fcm.googleapis.com/a"),
            "user info before an allowed host",
        ),
        (
            web("https://fcm.googleapis.com@evil.example/a"),
            "an allowed host as the user name of another",
        ),
        (
            web("https://evil.example\\@fcm.googleapis.com/a"),
            "a backslash ending the authority early",
        ),
        (
            web("https://fcm.googleapis.com\\@evil.example/a"),
            "a backslash after an allowed host",
        ),
        (
            device.endpoint(PushService::Fcm, "a-registration-token".into()),
            "FCM on a server without it",
        ),
        (
            be_block_protocol::PushEndpoint {
                p256dh: vec![4; 33],
                ..web("https://fcm.googleapis.com/fcm/send/a")
            },
            "a compressed key",
        ),
        (
            be_block_protocol::PushEndpoint {
                p256dh: [vec![4], vec![1; 64]].concat(),
                ..web("https://fcm.googleapis.com/fcm/send/a")
            },
            "a key off the curve",
        ),
    ] {
        let registered = member
            .call(|peer| client::notifications::register_endpoint(peer, endpoint))
            .await;
        if !refused_as(&registered, ErrorCode::InvalidRequest) {
            wrong.push((why, registered));
        }
    }
    assert!(wrong.is_empty(), "registered {wrong:?}");
    for address in [
        "https://fcm.googleapis.com/fcm/send/a",
        "https://FCM.googleapis.com/wp/b",
        "https://updates.push.services.mozilla.com/wpush/v2/c",
        "https://web.push.apple.com/d",
        "https://wns2-par02p.notify.windows.com/w/?token=e",
        "https://ntfy.example.org/upf",
    ] {
        member.register_endpoint(web(address)).await;
    }
    assert!(push::allowed_host("db5p.notify.windows.com", &[]));
    assert!(push::allowed_host(
        "autopush.push.services.mozilla.com",
        &[]
    ));
    assert!(!push::allowed_host("ntfy.example.org", &[]));

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
        ("::a00:1", false),
        ("2002:a00:1::1", false),
        ("2001::1", false),
        ("2001:db8::1", false),
        ("64:ff9b::a00:1", false),
        ("64:ff9b:1::1", false),
        ("fec0::1", false),
        ("100::1", false),
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
