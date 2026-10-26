use super::*;
use beui::Key;

fn press(test: &mut DocumentTest, key: Key) {
    test.frame(
        [true, false]
            .into_iter()
            .map(|pressed| {
                Input::Key(beui::Keystroke {
                    key: Some(key),
                    pressed,
                    ..Default::default()
                })
            })
            .collect(),
    );
    test.frame(Vec::new());
}

fn type_password(test: &mut DocumentTest, password: &str) {
    for letter in password.chars() {
        test.frame(vec![Input::Text(letter.to_string())]);
    }
    press(test, Key::Enter);
}

#[test]
fn the_lock_screen_sample_stays_locked_until_its_password_is_typed() {
    let mut test = alone(WIDE, Page::Overlays);

    test.click("demo.lock.open");
    test.frame(Vec::new());
    assert!(
        test.shows("demo.lock"),
        "the lock screen covers the window"
    );

    type_password(&mut test, "hunter2");
    assert!(test.shows("demo.lock.error"), "a wrong password says so");
    test.snapshot("lock_screen_after_a_wrong_password");

    press(&mut test, Key::Escape);
    test.click(pos2(4.0, 4.0));
    test.frame(Vec::new());
    assert!(
        test.shows("demo.lock"),
        "neither Escape nor a click outside the card unlocks"
    );

    type_password(&mut test, "beui");
    assert!(!test.shows("demo.lock"), "the right password unlocks");
}
