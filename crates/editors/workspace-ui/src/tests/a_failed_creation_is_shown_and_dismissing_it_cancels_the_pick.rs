use block_editor_beui::{BlockLocation, CreationProgress};

use super::*;

#[test]
fn a_failed_creation_is_shown_and_dismissing_it_cancels_the_pick() {
    let (mut fixture, _) = editor();
    fixture.test.catalog(catalog());
    let mut fixture = picking(&fixture, filter(), BlockLocation::Root);
    fixture.settle();
    fixture
        .test
        .click(format!("picker.menu.{TEMPLATE_EDITOR}/main"));
    fixture.settle();

    report_creation(
        &mut fixture,
        CreationProgress::Failed("the disk is full".to_owned()),
    );
    assert!(fixture.says("the disk is full"));
    assert!(fixture.test.take_picked().is_empty());

    fixture.click_text("Dismiss");

    assert_eq!(fixture.test.take_picked(), vec![Ok(None)]);
}
