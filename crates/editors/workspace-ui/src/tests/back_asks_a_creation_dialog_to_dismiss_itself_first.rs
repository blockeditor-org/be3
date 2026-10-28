use block_editor_beui::{BlockLocation, CreationProgress, DialogAnswer, DialogStatus};

use super::*;

#[test]
fn back_asks_a_creation_dialog_to_dismiss_itself_first() {
    let (mut fixture, _) = editor();
    let mut dialog = catalog();
    dialog.templates[0].dialog = true;
    fixture.test.catalog(dialog);
    let mut fixture = picking(&fixture, filter(), BlockLocation::Root);
    fixture.settle();
    fixture
        .test
        .click(format!("picker.menu.{TEMPLATE_EDITOR}/main"));
    fixture.settle();
    let asking = DialogStatus {
        handles_dismiss: true,
        answer: None,
    };
    report_creation_dialog(&mut fixture, CreationProgress::Options, asking);

    fixture.test.back();
    fixture.test.settle();
    assert_eq!(
        fixture.test.take_child_dismissals().len(),
        1,
        "back goes to the creation dialog rather than closing it"
    );
    assert!(fixture.test.shown("picker.creation"));
    assert!(fixture.test.take_picked().is_empty());

    report_creation_dialog(
        &mut fixture,
        CreationProgress::Options,
        DialogStatus {
            answer: Some(DialogAnswer::Dismissed),
            ..asking
        },
    );
    assert_eq!(
        fixture.test.take_picked(),
        vec![Ok(None)],
        "the dialog dismissing itself cancels the pick"
    );
    assert!(!fixture.test.shown("picker.creation"));
}
