use super::*;

#[test]
fn on_the_web_it_says_it_is_not_available() {
    let mut boat = Boat::opened_at("https://be3.example");

    assert!(boat.test.shown("boat.web"));
    assert!(!boat.test.shown("boat.sign-in"));
    assert!(!boat.test.has_fetches());
    boat.test.snapshot("on_the_web_it_says_it_is_not_available");
}
