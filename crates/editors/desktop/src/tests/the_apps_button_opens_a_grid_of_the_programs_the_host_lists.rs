use super::*;

#[test]
fn the_apps_button_opens_a_grid_of_the_programs_the_host_lists() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(!fixture.test.shown("desktop.apps.panel"));

    fixture.test.click("desktop.apps");
    fixture.settle();
    assert!(
        matches!(
            fixture
                .test
                .take_host_calls(block_ui_test::requested::program)[..],
            [ProgramAction::List { icon_size: 32 }]
        ),
        "opening the panel asks the host to list the programs, with icons at its scale"
    );
    assert!(
        fixture.test.wants_keyboard(),
        "the open panel asks the host for the keyboard"
    );
    fixture.test.set_watched::<ProgramsWatch>(&programs());
    fixture.settle();
    let files = fixture.test.rect_of("desktop.apps.item.files.desktop");
    let foot = fixture.test.rect_of("desktop.apps.item.foot.desktop");
    assert!(
        foot.min.x > files.min.x && (foot.min.y - files.min.y).abs() < 1.0,
        "the programs lie side by side in a grid: {files:?}, {foot:?}"
    );
    assert!(fixture.test.shown("desktop.apps.sessions.new"));
    assert!(fixture.test.shown("desktop.apps.menu"));
    fixture.test.snapshot("the_apps_panel");

    fixture.test.key(Key::ArrowRight, Modifiers::NONE);
    fixture.settle();
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::program),
        [ProgramAction::Launch {
            program: "foot.desktop".to_owned(),
            launch: None
        }],
        "the arrow keys walk the grid"
    );
    assert!(
        !fixture.test.shown("desktop.apps.panel"),
        "launching closes it"
    );

    fixture.test.click("desktop.apps");
    fixture.settle();
    fixture.test.type_text("shell");
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.apps.item.files.desktop"),
        "searching narrows the programs by their keywords"
    );
    fixture.test.snapshot("the_apps_panel_searching");
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(block_ui_test::requested::program),
        [
            ProgramAction::List { icon_size: 32 },
            ProgramAction::Launch {
                program: "foot.desktop".to_owned(),
                launch: None
            }
        ]
    );
    assert!(!fixture.test.shown("desktop.apps.panel"));
    assert!(!fixture.test.wants_keyboard());
}
