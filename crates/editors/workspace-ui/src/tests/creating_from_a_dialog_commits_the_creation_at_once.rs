use block_editor_beui::{BlockLocation, CreationProgress};

use super::*;

#[test]
fn creating_from_a_dialog_commits_the_creation_at_once() {
    let (mut fixture, _) = editor();
    let mut dialog = catalog();
    dialog.templates[0].dialog = true;
    fixture.test.catalog(dialog);
    let parent = Uuid::new_v4();
    fixture
        .test
        .request_pick(5, filter(), BlockLocation::Block(parent.into_bytes()));
    fixture.settle();
    fixture
        .test
        .click(&format!("picker.tile.{TEMPLATE_EDITOR}/main"));
    fixture.settle();
    fixture.settle();
    report_creation(&mut fixture, CreationProgress::Options { ready: true });

    fixture.test.click("picker.create");
    fixture.settle();

    assert_eq!(
        fixture
            .test
            .take_child_commits()
            .into_iter()
            .map(|(_, parent, name)| (parent, name))
            .collect::<Vec<_>>(),
        vec![(BlockLocation::Block(parent.into_bytes()), None)],
        "the creation is committed without waiting for its status to change"
    );
}
