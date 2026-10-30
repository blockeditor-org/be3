use super::*;
use block_editor_beui::ShellCommand;

#[test]
fn an_unreadable_layout_is_kept_until_the_person_resets_it() {
    let (mut fixture, _) = profiled_sized(Some(Vec2::new(900.0, 600.0)), Some(unreadable_layout()));

    assert!(
        fixture.test.shown("workspace.layout.reset"),
        "the person is asked what to do with the layout"
    );
    assert_eq!(
        profile(&fixture).state("layout"),
        Some(&unreadable_layout()),
        "the saved layout is not replaced while the notice is open"
    );
    fixture
        .test
        .snapshot("an_unreadable_layout_is_kept_until_the_person_resets_it");

    fixture.test.click("workspace.layout.copy-report");
    fixture.settle();
    let copied = fixture.test.take_copied_text();
    assert!(
        copied
            .iter()
            .any(|report| report.contains("could not be read")),
        "the report names the failure: {copied:?}"
    );

    fixture.test.take_shell_commands();
    fixture.test.click("workspace.layout.exit");
    fixture.settle();
    let workspace = fixture.test.block_id().expect("the workspace is a block");
    assert_eq!(
        fixture.test.take_shell_commands(),
        vec![(workspace, ShellCommand::CloseEditor)],
        "exiting asks the host to close the session itself"
    );
    assert_eq!(
        profile(&fixture).state("layout"),
        Some(&unreadable_layout()),
        "exiting leaves the saved layout as it was"
    );
}
