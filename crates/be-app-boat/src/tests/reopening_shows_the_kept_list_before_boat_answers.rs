use super::*;

#[test]
fn reopening_shows_the_kept_list_before_boat_answers() {
    let mut first = Boat::listing(SANDBOXES);
    first.test.presence_visible(false);

    let mut boat = first.reopened();

    assert!(
        !boat.test.shown("boat.sign-in"),
        "a kept key never shows the sign-in page"
    );
    assert!(boat.test.shown("boat.sandbox.bx_k7m2p9qa.detail"));
    assert_eq!(boat.label("boat.status"), "Updated just now");
    let reading = boat.fetch();
    assert_eq!(
        reading.request.url, LIST,
        "it reads the list again, with the kept key"
    );
}
