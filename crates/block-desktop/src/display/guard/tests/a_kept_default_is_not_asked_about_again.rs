use super::*;

#[test]
fn a_kept_default_is_not_asked_about_again() {
    let mut guard = showing(gaming_monitor(), DisplaySettings::default());
    assert!(guard.asking());
    assert_eq!(
        guard.keep(),
        vec![DisplaySettings::set_mode(MONITOR, Some(fast()))],
        "keeping the default saves it as the monitor's mode"
    );
    assert!(!guard.asking());

    let restarted = showing(gaming_monitor(), guard.applied().clone());
    assert!(!restarted.asking(), "the next start does not ask again");
}
