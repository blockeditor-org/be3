use super::*;

#[test]
fn power_maps_both_ways_and_windows_never_ends_the_session_itself() {
    let available = power_availability(be_win32::PowerAvailability {
        lock: true,
        suspend: false,
        restart: true,
        power_off: true,
        log_out: false,
    });
    assert!(available.lock && !available.suspend && available.restart);
    assert!(available.power_off && !available.log_out && !available.ending);
    for (ours, theirs) in [
        (
            block_plugin_api::PowerAction::Lock,
            be_win32::PowerAction::Lock,
        ),
        (
            block_plugin_api::PowerAction::Suspend,
            be_win32::PowerAction::Suspend,
        ),
        (
            block_plugin_api::PowerAction::Restart,
            be_win32::PowerAction::Restart,
        ),
        (
            block_plugin_api::PowerAction::PowerOff,
            be_win32::PowerAction::PowerOff,
        ),
        (
            block_plugin_api::PowerAction::LogOut,
            be_win32::PowerAction::LogOut,
        ),
    ] {
        assert_eq!(power_action(ours), theirs);
    }
}
