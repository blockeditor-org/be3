use super::*;

#[test]
fn a_mode_kept_here_is_not_asked_about_on_the_next_start() {
    let mut guard = showing(gaming_monitor(), saved(Some(slow())));
    guard.take(saved(Some(small())));
    guard.keep();

    let mut restarted = Guard::default();
    restarted.remember(guard.tried().clone());
    restarted.take(saved(Some(small())));
    restarted.set_screens(vec![gaming_monitor()]);
    assert!(!restarted.asking());

    let mut elsewhere = Guard::default();
    elsewhere.take(saved(Some(small())));
    elsewhere.set_screens(vec![gaming_monitor()]);
    assert!(
        elsewhere.asking(),
        "a device that never showed the mode asks, though the settings hold it"
    );
}
