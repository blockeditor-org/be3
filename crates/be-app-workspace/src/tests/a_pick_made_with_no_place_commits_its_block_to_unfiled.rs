use be_sdk_beui::{BlockLocation, CreationProgress, DialogAnswer, DialogStatus};

use super::*;

#[test]
fn a_pick_made_with_no_place_commits_its_block_to_unfiled() {
    let (mut fixture, _) = editor();
    let mut dialog = catalog();
    dialog.templates[0].dialog = true;
    fixture.test.catalog(dialog);
    let mut fixture = picking(&fixture, filter(), BlockLocation::Unfiled);
    fixture.settle();
    fixture.test.settle();
    fixture
        .test
        .snapshot("a_pick_with_no_place_offers_to_make_its_block_in_unfiled");
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
        vec![(BlockLocation::Unfiled, None)],
        "asked by an editor with no block of its own and no place, the new block goes to Unfiled"
    );
}
