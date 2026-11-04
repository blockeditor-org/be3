use super::*;

#[test]
fn suspending_asks_logind_at_once_and_only_when_it_can() {
    let mut control = Recorder {
        windows: 2,
        ..Recorder::default()
    };
    let mut power = Power::default();
    power.log_in();
    assert_eq!(
        power.availability(),
        PowerAvailability {
            log_out: true,
            session: true,
            ..PowerAvailability::default()
        },
        "before logind answers, only logging out is offered"
    );
    power.request(PowerAction::Suspend, &mut control);
    assert!(
        control.calls.is_empty(),
        "logind said nothing about suspending"
    );

    power.set_abilities(EVERYTHING);
    power.request(PowerAction::Suspend, &mut control);
    power.frame(&mut control);
    assert_eq!(control.calls, vec![LogindCall::Suspend]);
    assert_eq!(control.closes, 0, "suspending leaves the programs open");
    power.called(LogindCall::Suspend, Ok(()), &mut control);
    assert_eq!(control.exits, 0, "and the session running");
    assert!(power.availability().suspend, "and can suspend again");
}
