use super::*;

#[test]
fn reconnecting_replaces_the_key() {
    let mut boat = Boat::listing(SANDBOXES);
    boat.wait(Duration::from_secs(15));
    let poll = boat.fetch();
    boat.answer(&poll, 403, EXPIRED);

    boat.test.click("boat.reconnect");
    boat.test.run();
    boat.secret = NEW_SECRET;
    boat.connect();
    let me = boat.fetch();
    boat.answer(&me, 200, ME);

    assert_eq!(
        boat.kept_key().map(|key| key.secret),
        Some(NEW_SECRET.to_owned())
    );
    let list = boat.fetch();
    assert_eq!(list.request.url, LIST);
    boat.answer(&list, 200, SANDBOXES);
    assert!(!boat.test.shown("boat.key-notice"));
}
