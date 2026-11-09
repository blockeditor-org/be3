use super::*;

#[test]
fn an_expired_key_offers_reconnect() {
    let mut boat = Boat::listing(SANDBOXES);

    boat.wait(Duration::from_secs(15));
    let poll = boat.fetch();
    boat.answer(&poll, 403, EXPIRED);

    assert_eq!(
        boat.label("boat.key-notice"),
        "This boat.dev key has expired. Make a new one and reconnect."
    );
    assert!(boat.test.shown("boat.sandbox.bx_k7m2p9qa.detail"), "the last list stays");
    boat.wait(Duration::from_secs(60));
    assert!(!boat.test.has_fetches(), "an expired key is not tried again");
    boat.test.snapshot("an_expired_key_offers_reconnect");

    boat.test.click("boat.reconnect");
    boat.test.run();
    assert!(boat.test.shown("boat.sign-in"));
}
