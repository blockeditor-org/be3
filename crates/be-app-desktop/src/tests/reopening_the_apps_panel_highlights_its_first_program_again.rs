use super::*;

#[test]
fn reopening_the_apps_panel_highlights_its_first_program_again() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.set_watched::<ProgramsWatch>(&a_shelf_of_programs());
    fixture.test.click("desktop.apps");
    fixture.settle();
    fixture.test.key(Key::ArrowRight, Modifiers::NONE);
    fixture.test.key(Key::ArrowRight, Modifiers::NONE);
    fixture.settle();
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    assert!(!fixture.test.shown("desktop.apps.panel"));

    fixture.test.click("desktop.apps");
    fixture.settle();
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(be_sdk_test::requested::program)
            .last(),
        Some(&ProgramAction::Launch {
            program: "files.desktop".to_owned(),
            launch: None
        }),
        "the highlight starts over at the first program, as the query does"
    );

    fixture.test.click("desktop.apps");
    fixture.settle();
    fixture.test.key(Key::ArrowRight, Modifiers::NONE);
    fixture.settle();
    fixture.test.type_text("f");
    fixture.settle();
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(be_sdk_test::requested::program)
            .last(),
        Some(&ProgramAction::Launch {
            program: "files.desktop".to_owned(),
            launch: None
        }),
        "a changed query highlights its best match"
    );
}
