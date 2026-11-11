use super::*;
use be_block_protocol::PushService;

#[test]
fn a_push_hosts_cap_is_keyed_by_its_allowlist_entry_and_statuses_are_judged() {
    let device = TestDevice::new();
    let added = vec!["ntfy.example.org".to_owned()];
    let key = |address: &str| {
        push::push_host(
            &device.endpoint(PushService::WebPush, address.to_owned()),
            &added,
        )
    };
    // An exact host is its own key.
    assert_eq!(key("https://fcm.googleapis.com/a"), "fcm.googleapis.com");
    // Every subdomain of a suffix shares the suffix's key, so they share one cap.
    assert_eq!(key("https://a.notify.windows.com/x"), ".notify.windows.com");
    assert_eq!(key("https://b.notify.windows.com/y"), ".notify.windows.com");
    assert_eq!(
        key("https://autopush.push.services.mozilla.com/z"),
        ".push.services.mozilla.com"
    );
    // The operator's hosts are their own keys, and anything else shares one.
    assert_eq!(key("https://NTFY.example.org./u"), "ntfy.example.org");
    assert_eq!(key("https://elsewhere.example/u"), "other");
    assert_eq!(
        push::push_host(&device.endpoint(PushService::Fcm, "token".into()), &added),
        "fcm"
    );

    for (status, outcome) in [
        (201, "delivered"),
        (404, "gone"),
        (410, "gone"),
        (401, "throttled"),
        (403, "throttled"),
        (413, "throttled"),
        (429, "throttled"),
        (500, "throttled"),
        (502, "throttled"),
        (503, "throttled"),
        (504, "throttled"),
        (302, "failed"),
        (400, "failed"),
    ] {
        let judged = match push::status_outcome(status) {
            push::Sent::Delivered => "delivered",
            push::Sent::Gone => "gone",
            push::Sent::Throttled(_) => "throttled",
            push::Sent::Failed(_) => "failed",
        };
        assert_eq!(judged, outcome, "a push service answering {status}");
    }
}
