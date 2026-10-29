use super::*;

#[test]
fn beuis_keyboard_is_used_only_where_the_platform_has_none_of_its_own() {
    let mut intaken = Intaken::new();
    intaken.frame(1.0, []);
    assert!(
        intaken.runner.context().keyboard_enabled(),
        "a platform with no keyboard of its own shows beui's"
    );

    let mut intaken = Intaken::new();
    intaken.platform.shows_keyboard = true;
    intaken.frame(1.0, []);
    assert!(
        !intaken.runner.context().keyboard_enabled(),
        "a platform with a keyboard of its own (Android, the web, a plugin's host) shows that"
    );
}
