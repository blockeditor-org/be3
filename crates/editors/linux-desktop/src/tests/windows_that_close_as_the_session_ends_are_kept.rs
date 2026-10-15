use super::*;

#[test]
fn windows_that_close_as_the_session_ends_are_kept() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.allow_power(EVERYTHING);
    fixture.show_windows(&[foot(1), notes(2)]);

    fixture.allow_power(PowerAvailability {
        restart: false,
        power_off: false,
        log_out: false,
        ending: true,
        ..EVERYTHING
    });
    fixture.show_windows(&[]);

    assert_eq!(
        kept_keys(&fixture),
        vec![1, 2],
        "the programs closing because the session ends are still there to bring back"
    );
}
