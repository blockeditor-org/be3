use super::*;
use beui::{Event, Key, Modifiers};

fn press(test: &mut DocumentTest, key: Key) {
    test.frame(
        [true, false]
            .into_iter()
            .map(|pressed| Event::Key {
                key,
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            })
            .collect(),
    );
    test.frame(Vec::new());
}

fn type_password(test: &mut DocumentTest, password: &str) {
    for letter in password.chars() {
        test.frame(vec![Event::Text(letter.to_string())]);
    }
    press(test, Key::Enter);
}

#[test]
fn the_lock_screen_sample_stays_locked_until_its_password_is_typed() {
    let mut test = alone(WIDE, Page::Overlays);

    test.click("demo.lock.open");
    test.frame(Vec::new());
    assert!(
        test.shows("demo.lock.0"),
        "the lock screen covers the window"
    );

    type_password(&mut test, "hunter2");
    assert!(test.shows("demo.lock.0.error"), "a wrong password says so");
    test.snapshot("lock_screen_after_a_wrong_password");

    press(&mut test, Key::Escape);
    test.click_at(pos2(4.0, 4.0));
    test.frame(Vec::new());
    assert!(
        test.shows("demo.lock.0"),
        "neither Escape nor a click outside the card unlocks"
    );

    type_password(&mut test, "beui");
    assert!(!test.shows("demo.lock.0"), "the right password unlocks");
}
