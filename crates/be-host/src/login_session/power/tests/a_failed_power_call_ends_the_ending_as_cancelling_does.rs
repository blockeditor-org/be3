use super::*;

#[test]
fn a_failed_power_call_ends_the_ending_as_cancelling_does() {
    let mut control = Recorder {
        windows: 1,
        ..Recorder::default()
    };
    let mut power = power();

    power.request(PowerAction::PowerOff, &mut control);
    power.frame(&mut control);
    control.windows = 0;
    power.frame(&mut control);
    assert_eq!(control.calls, vec![LogindCall::PowerOff]);
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
    assert_eq!(
        power.availability().ending,
        None,
        "the shell is told the session goes on, so it keeps what closed as placeholders"
    );
    assert!(
        power.availability().power_off,
        "it can be tried again from the menu"
    );
}
