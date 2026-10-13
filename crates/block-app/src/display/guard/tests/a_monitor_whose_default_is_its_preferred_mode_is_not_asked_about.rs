use super::*;

#[test]
fn a_monitor_whose_default_is_its_preferred_mode_is_not_asked_about() {
    let guard = showing(office_monitor(), DisplaySettings::default());
    assert!(!guard.asking());
}
