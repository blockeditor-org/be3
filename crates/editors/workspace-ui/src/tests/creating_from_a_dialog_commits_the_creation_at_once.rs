use block_editor_beui::{BlockLocation, CreationProgress, DialogAnswer, DialogStatus};

use super::*;

#[test]
fn creating_from_a_dialog_commits_the_creation_at_once() {
    let (mut fixture, _) = editor();
    let mut dialog = catalog();
    dialog.templates[0].dialog = true;
    fixture.test.catalog(dialog);
    let parent = Uuid::new_v4();
    let mut fixture = picking(&fixture, filter(), BlockLocation::Block(parent.into_bytes()));
    fixture.settle();
    fixture
        .test
        .click(format!("picker.menu.{TEMPLATE_EDITOR}/main"));
    fixture.settle();
    fixture.settle();

    report_creation_dialog(
        &mut fixture,
        CreationProgress::Options,
        DialogStatus {
            handles_dismiss: true,
            answer: Some(DialogAnswer::Accepted),
        },
    );

    assert_eq!(
        fixture
            .test
            .take_child_commits()
            .into_iter()
            .map(|(_, parent, name)| (parent, name))
            .collect::<Vec<_>>(),
        vec![(BlockLocation::Block(parent.into_bytes()), None)],
        "the dialog accepting itself commits the creation without waiting for its status to change"
    );
}
