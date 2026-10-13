use super::*;

#[test]
fn the_session_ends_after_the_grace_period_even_if_a_program_stays() {
    let mut control = Recorder {
        windows: 1,
        ..Recorder::default()
    };
    let mut power = power();

    power.request(PowerAction::LogOut, seconds(0), &mut control);
    assert!(power.frame(GRACE - seconds(1), &mut control).is_some());
    assert_eq!(control.exits, 0);

    assert_eq!(power.frame(GRACE, &mut control), None);
    assert_eq!(
        control.exits, 1,
        "a program that will not close is left behind"
    );
    power.frame(GRACE + seconds(1), &mut control);
    assert_eq!(control.exits, 1, "and the session ends once");
}
