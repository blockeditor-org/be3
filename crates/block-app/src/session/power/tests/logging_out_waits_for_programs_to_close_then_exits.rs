use super::*;

#[test]
fn logging_out_waits_for_programs_to_close_then_exits() {
    let mut control = Recorder {
        windows: 2,
        ..Recorder::default()
    };
    let mut power = power();

    let wait = power.request(PowerAction::LogOut, seconds(10), &mut control);
    assert_eq!(control.closes, 1, "every program is asked to close");
    assert_eq!(wait, Some(GRACE), "and given the grace period to do it");
    assert_eq!(control.exits, 0);
    assert_eq!(
        power.availability(),
        PowerAvailability::default(),
        "nothing else can be asked for while the session ends"
    );

    control.windows = 1;
    assert_eq!(
        power.frame(seconds(11), &mut control),
        Some(GRACE - seconds(1))
    );
    assert_eq!(control.exits, 0, "one program is still closing");

    control.windows = 0;
    assert_eq!(power.frame(seconds(12), &mut control), None);
    assert_eq!(control.exits, 1, "the session ends once they have all gone");
    assert!(
        control.calls.is_empty(),
        "logging out does not touch logind"
    );
}
