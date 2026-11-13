use super::*;

#[test]
fn a_query_no_program_matches_runs_as_a_command() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.set_watched::<ProgramsWatch>(&programs());
    fixture.test.click("desktop.apps");
    fixture.settle();
    fixture.test.type_text("weston-simple-shm");
    fixture.settle();
    assert!(fixture.test.shown("desktop.apps.item.run"));
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(be_sdk_test::requested::program)
            .last(),
        Some(&ProgramAction::Exec {
            command: vec![
                "sh".to_owned(),
                "-c".to_owned(),
                "weston-simple-shm".to_owned()
            ],
            launch: None,
        }),
        "the typed line runs through the shell the host names"
    );
    assert!(!fixture.test.shown("desktop.apps.panel"));
}
