use super::*;

#[test]
fn the_list_shows_each_sandbox_and_how_old_it_is() {
    let mut boat = Boat::listing(SANDBOXES);

    assert_eq!(boat.label("boat.status"), "Updated just now");
    assert_eq!(
        boat.label("boat.sandbox.bx_k7m2p9qa.detail"),
        "Running · default, 4 vCPU, 8 GB · 203.0.113.10:19035"
    );
    assert_eq!(
        boat.label("boat.sandbox.bx_r4t8w3ne.detail"),
        "Stopped · small, 2 vCPU, 4 GB"
    );
    assert!(boat.test.shown("boat.sandbox.bx_k7m2p9qa.stop"));
    assert!(boat.test.shown("boat.sandbox.bx_r4t8w3ne.start"));
    boat.test.snapshot("the_list_shows_each_sandbox_and_how_old_it_is");

    boat.wait(Duration::from_secs(15));
    let again = boat.fetch();
    assert_eq!(again.request.url, LIST, "the list is read again while on screen");
    boat.answer(&again, 200, SANDBOXES);

    boat.test.presence_visible(false);
    boat.wait(Duration::from_secs(60));
    assert!(!boat.test.has_fetches(), "off screen, nothing is polled");
}
