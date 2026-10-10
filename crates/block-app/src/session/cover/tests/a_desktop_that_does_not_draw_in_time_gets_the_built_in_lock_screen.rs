use super::*;

#[test]
fn a_desktop_that_does_not_draw_in_time_gets_the_built_in_lock_screen() {
    let mut choice = CoverChoice::default();
    assert_eq!(choice.choose(true, millis(0), STARTING).0, Cover::Blank);
    assert_eq!(
        choice.choose(true, DRAW_WITHIN - millis(1), STARTING).0,
        Cover::Blank
    );
    let (cover, wake) = choice.choose(true, DRAW_WITHIN, STARTING);
    assert_eq!(cover, Cover::Fallback);
    assert_eq!(wake, None);
    assert!(!cover.takes_desktop_passwords());

    assert_eq!(
        choice.choose(true, DRAW_WITHIN + millis(1), DRAWN).0,
        Cover::Fallback,
        "a drawing that comes late does not take over from the built-in lock screen"
    );
}
