use super::*;

#[test]
fn unsent_edits_can_be_restored_or_discarded() {
    let mut shown = Shown::unsent(unsent_view());
    shown.painted("unsent_edits_can_be_restored_or_discarded");
    let first = uuid::Uuid::from_u128(1);
    shown.stepped.click(beui::testing::Target::id(format!(
        "app.unsent.restore.{first}"
    )));
    shown.stepped.click(beui::testing::Target::id(format!(
        "app.unsent.discard.{first}"
    )));
    let commands = crate::ui::take_commands();
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, UiCommand::RestoreUnsent(block) if *block == first))
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, UiCommand::DiscardUnsent(block) if *block == first)),
        "discarding asks first rather than deleting at once"
    );
    let second = uuid::Uuid::from_u128(2);
    shown.stepped.click(beui::testing::Target::id(format!(
        "app.unsent.restore.{second}"
    )));
    assert!(
        crate::ui::take_commands()
            .iter()
            .all(|command| !matches!(command, UiCommand::RestoreUnsent(_))),
        "an edit to a block that cannot be edited now is not offered back"
    );
}
