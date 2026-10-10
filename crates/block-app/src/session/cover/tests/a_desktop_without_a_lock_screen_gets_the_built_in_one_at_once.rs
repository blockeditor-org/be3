use super::*;

#[test]
fn a_desktop_without_a_lock_screen_gets_the_built_in_one_at_once() {
    let mut choice = CoverChoice::default();
    assert_eq!(
        choice.choose(true, millis(0), PluginLock::default()).0,
        Cover::Fallback
    );
    let mut choice = CoverChoice::default();
    let undeclared = PluginLock {
        offered: false,
        ..DRAWN
    };
    assert_eq!(
        choice.choose(true, millis(0), undeclared).0,
        Cover::Fallback,
        "a plugin that declares no lock region is not trusted with one"
    );
}
