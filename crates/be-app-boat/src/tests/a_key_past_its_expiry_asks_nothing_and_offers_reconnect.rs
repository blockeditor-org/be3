use super::*;

#[test]
fn a_key_past_its_expiry_asks_nothing_and_offers_reconnect() {
    let mut boat = Boat::listing(SANDBOXES);

    boat.wait(Duration::from_secs(366 * 24 * 60 * 60));
    boat.wait(Duration::from_secs(60));

    assert!(!boat.test.has_fetches());
    assert!(boat.label("boat.key-notice").contains("has expired"));
}
