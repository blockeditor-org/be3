use super::*;

#[test]
fn power_maps_both_ways_and_windows_never_ends_the_session_itself() {
    let available = power_availability(be_host_win32::PowerAvailability {
        lock: true,
        suspend: false,
        restart: true,
        power_off: true,
        log_out: false,
    });
    assert!(available.lock && !available.suspend && available.restart);
    assert!(available.power_off && !available.log_out && !available.ending);
    assert!(available.session, "windows is always the login session");
    for (ours, theirs) in [
        (
            be_sdk_api::PowerAction::Lock,
            be_host_win32::PowerAction::Lock,
        ),
        (
            be_sdk_api::PowerAction::Suspend,
            be_host_win32::PowerAction::Suspend,
        ),
        (
            be_sdk_api::PowerAction::Restart,
            be_host_win32::PowerAction::Restart,
        ),
        (
            be_sdk_api::PowerAction::PowerOff,
            be_host_win32::PowerAction::PowerOff,
        ),
        (
            be_sdk_api::PowerAction::LogOut,
            be_host_win32::PowerAction::LogOut,
        ),
    ] {
        assert_eq!(power_action(ours), theirs);
    }
}
