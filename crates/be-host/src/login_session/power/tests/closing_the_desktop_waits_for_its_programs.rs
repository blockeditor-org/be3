use super::*;

#[test]
fn closing_the_desktop_waits_for_its_programs() {
    let mut control = Recorder::default();
    let mut power = Power::default();
    assert!(
        power.close(&mut control),
        "with no programs it closes at once"
    );

    control.windows = 2;
    assert!(!power.close(&mut control), "with programs it waits");
    assert_eq!(power.availability().ending, Some(SessionEnd::Close));
    assert!(!power.close(&mut control), "asking again waits the same");
    power.frame(&mut control);
    assert_eq!(control.closes, 1, "the programs are asked to close once");

    control.windows = 0;
    power.frame(&mut control);
    assert_eq!(control.exits, 1);
    assert!(
        power.close(&mut control),
        "the close it asks for goes through"
    );
    power.stay();
    control.windows = 1;
    assert!(
        !power.close(&mut control),
        "when be-host stays open after all, a later close waits again"
    );

    let mut logged_in = Power::default();
    logged_in.log_in();
    logged_in.close(&mut control);
    assert_eq!(
        logged_in.availability().ending,
        Some(SessionEnd::LogOut),
        "in a login session closing is logging out"
    );
}
