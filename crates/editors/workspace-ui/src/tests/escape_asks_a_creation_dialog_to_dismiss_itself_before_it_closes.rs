use block_editor_beui::{BlockLocation, CreationProgress, DialogAnswer, DialogStatus};
use block_editor_beui::beui::{Key, Modifiers};

use super::*;

#[test]
fn escape_asks_a_creation_dialog_to_dismiss_itself_before_it_closes() {
    let (mut fixture, _) = editor();
    let mut dialog = catalog();
    dialog.templates[0].dialog = true;
    fixture.test.catalog(dialog);
    fixture
        .test
        .request_pick(5, filter(), BlockLocation::Root, None);
    fixture.settle();
    fixture
        .test
        .click(format!("picker.menu.{TEMPLATE_EDITOR}/main"));
    fixture.settle();
    fixture.settle();
    let asking = DialogStatus {
        handles_dismiss: true,
        answer: None,
    };
    report_creation_dialog(&mut fixture, CreationProgress::Options, asking);

    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture.test.take_child_dismissals().len(),
        1,
        "Escape goes to the creation dialog rather than closing it"
    );
    assert!(fixture.test.shown("picker.creation"));
    assert!(fixture.test.take_pick_answers().is_empty());

    report_creation_dialog(
        &mut fixture,
        CreationProgress::Options,
        DialogStatus {
            answer: Some(DialogAnswer::Dismissed),
            ..asking
        },
    );
    assert_eq!(
        fixture.test.take_pick_answers(),
        vec![(5, Ok(None))],
        "the dialog dismissing itself cancels the pick"
    );
    assert!(!fixture.test.shown("picker.creation"));
}
