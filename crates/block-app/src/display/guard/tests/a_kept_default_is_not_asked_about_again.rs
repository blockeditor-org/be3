use super::*;

#[test]
fn a_kept_default_is_not_asked_about_again() {
    let mut guard = showing(gaming_monitor(), DisplaySettings::default());
    assert!(guard.asking());
    assert_eq!(
        guard.keep(),
        vec![change(Some(fast()))],
        "keeping the default saves it as the monitor's mode"
    );
    assert!(!guard.asking());

    let mut restarted = Guard::default();
    restarted.remember(guard.tried().clone());
    restarted.take(guard.applied().clone());
    restarted.set_screens(vec![gaming_monitor()]);
    assert!(!restarted.asking(), "the next start does not ask again");
}
