use super::*;

#[test]
fn on_the_web_it_says_it_is_not_available() {
    let mut boat = Boat::new();
    boat.test
        .set_watched::<PageOriginWatch>(&"https://be3.example".to_owned());
    boat.test.run();

    assert!(boat.test.shown("boat.web"));
    assert!(!boat.test.shown("boat.sign-in"));
    assert!(!boat.test.has_fetches());
    boat.test.snapshot("on_the_web_it_says_it_is_not_available");
}
