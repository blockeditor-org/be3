use super::*;

#[test]
fn the_programs_button_opens_the_launcher_on_the_programs_the_host_lists() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(!fixture.test.shown("launcher"));

    fixture.test.click("desktop.launcher");
    fixture.settle();
    assert!(
        matches!(
            fixture.test.take_actions::<ProgramAction>()[..],
            [ProgramAction::List { icon_size: 32 }]
        ),
        "opening the launcher asks the host to list the programs, with icons at its scale"
    );
    assert!(
        fixture.test.wants_keyboard(),
        "the open launcher asks the host for the keyboard"
    );
    fixture.test.set_host_value::<Programs>(&programs());
    fixture.settle();
    assert!(fixture.test.shown("launcher.item.foot.desktop"));
    assert!(fixture.test.shown("launcher.item.files.desktop"));
    fixture.test.snapshot("the_program_launcher");

    fixture.test.text("shell");
    fixture.settle();
    assert!(
        !fixture.test.shown("launcher.item.files.desktop"),
        "searching narrows the programs by their keywords"
    );
    fixture.test.key_press(Key::Enter);
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<ProgramAction>(),
        [ProgramAction::Launch("foot.desktop".to_owned())]
    );
    assert!(!fixture.test.shown("launcher"), "launching closes it");
    assert!(!fixture.test.wants_keyboard());
}
