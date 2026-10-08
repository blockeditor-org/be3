use super::*;

#[test]
fn a_change_to_a_monitor_that_is_not_connected_is_not_asked_about() {
    let mut guard = showing(office_monitor(), DisplaySettings::default());
    let elsewhere = edited(
        &DisplaySettings::default(),
        &[DisplaySettings::set_mode("HDMI-A-2", Some(small()))],
    );
    assert!(guard.take(elsewhere), "it is saved at once");
    assert!(!guard.asking());

    let mut windowed = Guard::default();
    assert!(
        windowed.take(saved(Some(small()))),
        "without a monitor to show it on, nothing is asked"
    );
    assert!(!windowed.asking());
}
