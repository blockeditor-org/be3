use super::*;

#[test]
fn a_desktop_that_stops_gets_the_built_in_lock_screen_until_the_next_lock() {
    let mut choice = CoverChoice::default();
    assert_eq!(choice.choose(true, millis(0), DRAWN).0, Cover::Plugin);
    let failed = PluginLock {
        failed: true,
        drawn: false,
        ..DRAWN
    };
    assert_eq!(choice.choose(true, millis(10), failed).0, Cover::Fallback);
    assert_eq!(
        choice.choose(true, millis(20), DRAWN).0,
        Cover::Fallback,
        "a restarted desktop does not take over from the built-in lock screen"
    );

    assert_eq!(choice.choose(false, millis(30), DRAWN).0, Cover::Blank);
    assert_eq!(
        choice.choose(true, millis(40), DRAWN).0,
        Cover::Plugin,
        "the next lock trusts the desktop again"
    );
}
