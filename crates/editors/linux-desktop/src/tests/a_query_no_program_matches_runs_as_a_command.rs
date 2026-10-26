use super::*;

#[test]
fn a_query_no_program_matches_runs_as_a_command() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.set_watched::<ProgramsWatch>(&programs());
    fixture.test.click("desktop.launcher");
    fixture.settle();
    fixture.test.type_text("weston-simple-shm");
    fixture.settle();
    assert!(fixture.test.shown("launcher.item.run"));
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::program)
            .last(),
        Some(&ProgramAction::Run("weston-simple-shm".to_owned()))
    );
    assert!(!fixture.test.shown("launcher"));
}
