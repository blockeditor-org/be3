use super::*;

#[test]
fn a_desktop_that_draws_its_lock_screen_is_shown_and_given_the_password() {
    let mut choice = CoverChoice::default();
    assert_eq!(choice.choose(false, millis(0), DRAWN), (Cover::Blank, None));

    let (cover, wake) = choice.choose(true, millis(100), STARTING);
    assert_eq!(cover, Cover::Blank, "only the opaque cover shows at first");
    assert_eq!(wake, Some(DRAW_WITHIN), "and the host looks again in time");
    assert!(!cover.takes_desktop_passwords());

    let (cover, wake) = choice.choose(true, millis(400), DRAWN);
    assert_eq!(
        cover,
        Cover::Plugin,
        "the desktop's lock screen shows once drawn"
    );
    assert_eq!(wake, None);
    assert!(cover.takes_desktop_passwords());

    let (cover, wake) = choice.choose(
        true,
        millis(500),
        PluginLock {
            unanswered: Some(millis(1000)),
            ..DRAWN
        },
    );
    assert_eq!(cover, Cover::Plugin, "a slow frame is not yet a hang");
    assert_eq!(wake, Some(HUNG_AFTER - millis(1000)));

    assert_eq!(choice.choose(false, millis(600), DRAWN).0, Cover::Blank);
    assert_eq!(
        choice.choose(true, millis(700), STARTING).0,
        Cover::Blank,
        "each lock waits for a drawing of its own"
    );
}
