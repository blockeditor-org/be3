use super::*;

#[test]
fn a_redirect_from_the_build_server_means_it_needs_waking() {
    // Caddy refusing a request or NativeLink answering a plain GET: the scheduler is there.
    assert!(nativelink::answers(404));
    assert!(nativelink::answers(200));
    // Boat answers a stopped sandbox's hosted port with a redirect to its fallback page.
    assert!(!nativelink::answers(302));
    assert!(!nativelink::answers(307));
    // Boat's edge with nothing behind it yet, while a resumed scheduler starts.
    assert!(!nativelink::answers(502));
}
