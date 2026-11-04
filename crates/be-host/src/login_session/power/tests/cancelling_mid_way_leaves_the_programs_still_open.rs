use super::*;

#[test]
fn cancelling_mid_way_leaves_the_programs_still_open() {
    let mut control = Recorder {
        windows: 3,
        ..Recorder::default()
    };
    let mut power = power();

    power.request(PowerAction::Restart, &mut control);
    power.frame(&mut control);
    control.windows = 1;
    power.frame(&mut control);
    power.answer(EndingAnswer::Cancel);
    assert_eq!(power.availability().ending, None);
    assert!(power.availability().restart, "it can be asked for again");

    control.windows = 0;
    power.frame(&mut control);
    assert!(
        control.calls.is_empty() && control.exits == 0,
        "the last program closing later ends nothing"
    );
    assert_eq!(control.closes, 1);
}
