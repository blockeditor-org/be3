use super::*;

#[test]
fn a_key_near_its_expiry_warns_two_weeks_before() {
    let mut boat = Boat::listing(SANDBOXES);
    assert!(!boat.test.shown("boat.key-notice"));

    boat.wait(Duration::from_secs(352 * 24 * 60 * 60));
    let poll = boat.fetch();
    boat.answer(&poll, 200, SANDBOXES);

    assert!(
        boat.label("boat.key-notice")
            .starts_with("This boat.dev key expires on "),
        "{}",
        boat.label("boat.key-notice")
    );
}
