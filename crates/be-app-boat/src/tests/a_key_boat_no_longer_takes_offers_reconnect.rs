use super::*;

#[test]
fn a_key_boat_no_longer_takes_offers_reconnect() {
    let mut boat = Boat::listing(SANDBOXES);

    boat.wait(Duration::from_secs(15));
    let poll = boat.fetch();
    boat.answer(&poll, 401, UNAUTHORIZED);

    assert_eq!(
        boat.label("boat.key-notice"),
        "boat.dev no longer accepts this key. Make a new one and reconnect."
    );
    boat.wait(Duration::from_secs(60));
    assert!(!boat.test.has_fetches(), "a refused key is not tried again");
}
