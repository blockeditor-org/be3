use super::*;

#[test]
fn a_program_that_never_closes_holds_the_session_until_it_is_ended_now() {
    for (action, call) in [
        (PowerAction::LogOut, None),
        (PowerAction::Restart, Some(LogindCall::Reboot)),
    ] {
        let mut control = Recorder {
            windows: 1,
            ..Recorder::default()
        };
        let mut power = power();

        power.request(action, &mut control);
        for _ in 0..10 {
            power.frame(&mut control);
        }
        assert_eq!(
            control.exits, 0,
            "a program asking about unsaved changes is waited for"
        );
        assert!(control.calls.is_empty(), "however long it takes");

        power.answer(EndingAnswer::EndNow);
        power.frame(&mut control);
        match call {
            None => assert_eq!(control.exits, 1, "ending now leaves it behind"),
            Some(call) => assert_eq!(control.calls, vec![call], "logind is asked with it open"),
        }
    }
}
