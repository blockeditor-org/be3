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

        power.request(action, &mut control);
        power.frame(&mut control);
        assert_eq!(control.closes, 1);
        assert!(
            control.calls.is_empty(),
            "logind waits for the programs to close"
        );

        control.windows = 0;
        power.frame(&mut control);
        assert_eq!(control.calls, vec![call]);
        power.frame(&mut control);
        assert_eq!(control.calls, vec![call], "logind is asked once");
        power.answer(EndingAnswer::Cancel);
        assert!(
            power.availability().ending.is_some(),
            "once logind is asked it is too late to cancel"
        );
        assert_eq!(control.exits, 0, "the session waits for logind's answer");

        power.called(call, Ok(()), &mut control);
        assert_eq!(control.exits, 1, "then ends");
    }
}
