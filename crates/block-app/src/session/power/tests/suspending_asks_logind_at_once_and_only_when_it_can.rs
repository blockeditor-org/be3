use super::*;

#[test]
fn suspending_asks_logind_at_once_and_only_when_it_can() {
    let mut control = Recorder {
        windows: 2,
        ..Recorder::default()
    };
    let mut power = Power::default();
    assert_eq!(
        power.availability(),
        PowerAvailability {
            log_out: true,
            ..PowerAvailability::default()
        },
        "before logind answers, only logging out is offered"
    );
    power.request(PowerAction::Suspend, seconds(1), &mut control);
    assert!(
        control.calls.is_empty(),
        "logind said nothing about suspending"
    );

    power.set_abilities(EVERYTHING);
    assert_eq!(
        power.request(PowerAction::Suspend, seconds(2), &mut control),
        None
    );
    assert_eq!(control.calls, vec![LogindCall::Suspend]);
    assert_eq!(control.closes, 0, "suspending leaves the programs open");
    power.called(LogindCall::Suspend, Ok(()), &mut control);
    assert_eq!(control.exits, 0, "and the session running");
    assert!(power.availability().suspend, "and can suspend again");
}
