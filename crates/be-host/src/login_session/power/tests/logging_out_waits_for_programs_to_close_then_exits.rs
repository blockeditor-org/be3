use super::*;

#[test]
fn logging_out_waits_for_programs_to_close_then_exits() {
    let mut control = Recorder {
        windows: 2,
        ..Recorder::default()
    };
    let mut power = power();

    power.request(PowerAction::LogOut, &mut control);
    assert_eq!(
        power.availability(),
        PowerAvailability {
            session: true,
            ending: Some(SessionEnd::LogOut),
            ..PowerAvailability::default()
        },
        "the shell is told the session ends, and nothing else can be asked for meanwhile"
    );
    assert_eq!(
        control.closes, 0,
        "the programs are asked to close only once the shell can have heard"
    );

    power.frame(&mut control);
    assert_eq!(control.closes, 1, "every program is asked to close");
    control.windows = 1;
    power.frame(&mut control);
    assert_eq!(control.closes, 1, "and asked once");
    assert_eq!(control.exits, 0, "one program is still closing");

    control.windows = 0;
    power.frame(&mut control);
    assert_eq!(control.exits, 1, "the session ends once they have all gone");
    assert!(
        control.calls.is_empty(),
        "logging out does not touch logind"
    );
    assert!(
        power.close(&mut control),
        "be-host's own close then goes through"
    );
}
