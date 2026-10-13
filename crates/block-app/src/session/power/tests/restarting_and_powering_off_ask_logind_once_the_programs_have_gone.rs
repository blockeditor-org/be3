use super::*;

#[test]
fn restarting_and_powering_off_ask_logind_once_the_programs_have_gone() {
    for (action, call) in [
        (PowerAction::Restart, LogindCall::Reboot),
        (PowerAction::PowerOff, LogindCall::PowerOff),
    ] {
        let mut control = Recorder {
            windows: 1,
            ..Recorder::default()
        };
        let mut power = power();

        power.request(action, seconds(0), &mut control);
        assert_eq!(control.closes, 1);
        assert!(
            control.calls.is_empty(),
            "logind waits for the programs to close"
        );

        control.windows = 0;
        power.frame(seconds(1), &mut control);
        assert_eq!(control.calls, vec![call]);
        power.frame(seconds(2), &mut control);
        assert_eq!(control.calls, vec![call], "logind is asked once");
        assert_eq!(control.exits, 0, "the session waits for logind's answer");

        power.called(call, Ok(()), &mut control);
        assert_eq!(control.exits, 1, "then ends");
    }
}
