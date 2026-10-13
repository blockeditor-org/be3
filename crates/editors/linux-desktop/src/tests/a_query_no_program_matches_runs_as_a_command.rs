use super::*;

#[test]
fn a_query_no_program_matches_runs_as_a_command() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.set_host_value::<Programs>(&programs());
    fixture.test.click("desktop.launcher");
    fixture.settle();
    fixture.test.text("weston-simple-shm");
    fixture.settle();
    assert!(fixture.test.shown("launcher.run"));
    fixture.test.key_press(Key::Enter);
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<ProgramAction>().last(),
        Some(&ProgramAction::Run("weston-simple-shm".to_owned()))
    );
    assert!(!fixture.test.shown("launcher"));
}
