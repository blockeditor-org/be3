use super::*;

#[test]
fn a_failed_power_off_is_reported_and_leaves_the_session_running() {
    let mut control = Recorder::default();
    let mut power = power();

    power.request(PowerAction::PowerOff, seconds(0), &mut control);
    assert_eq!(
        control.calls,
        vec![LogindCall::PowerOff],
        "with no programs open there is nothing to wait for"
    );
    power.called(
        LogindCall::PowerOff,
        Err("Interactive authentication required.".to_owned()),
        &mut control,
    );
    assert_eq!(control.exits, 0);
    assert_eq!(
        control.problems,
        vec!["Could not power off: Interactive authentication required.".to_owned()]
    );
    assert!(
        power.availability().power_off,
        "it can be tried again from the menu"
    );
}
