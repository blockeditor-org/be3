use super::*;

#[test]
fn a_desktop_that_stops_answering_gets_the_built_in_lock_screen() {
    let mut choice = CoverChoice::default();
    assert_eq!(choice.choose(true, millis(0), DRAWN).0, Cover::Plugin);
    let hung = PluginLock {
        unanswered: Some(HUNG_AFTER),
        ..DRAWN
    };
    assert_eq!(choice.choose(true, millis(5000), hung).0, Cover::Fallback);
    assert_eq!(
        choice.choose(true, millis(6000), DRAWN).0,
        Cover::Fallback,
        "the desktop answering again changes nothing until the screen is unlocked"
    );

    let mut choice = CoverChoice::default();
    let already_hung = PluginLock {
        unanswered: Some(HUNG_AFTER),
        ..STARTING
    };
    assert_eq!(
        choice.choose(true, millis(0), already_hung).0,
        Cover::Fallback,
        "a desktop already hung when the screen locks is not waited for"
    );
}
