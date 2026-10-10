use super::*;

#[test]
fn reverting_a_change_leaves_monitors_that_are_not_connected_alone() {
    let mut guard = showing(gaming_monitor(), saved(Some(slow())));
    guard.take(saved(Some(small())));
    let elsewhere = edited(
        &saved(Some(small())),
        &[DisplaySettings::set_mode("HDMI-A-2", Some(small()))],
    );
    assert!(!guard.take(elsewhere), "the connected monitor still waits");

    assert_eq!(
        guard.revert(),
        vec![DisplaySettings::set_mode(MONITOR, Some(slow()))],
        "only the connected monitor is put back"
    );
    assert_eq!(guard.applied().mode("HDMI-A-2"), Some(small()));
}
