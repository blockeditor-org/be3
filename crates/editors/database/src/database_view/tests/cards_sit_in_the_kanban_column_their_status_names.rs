use super::*;

#[test]
fn cards_sit_in_the_kanban_column_their_status_names() {
    let mut fixture = editor(&[
        ("Status", DatabaseFieldType::Enum),
        ("Name", DatabaseFieldType::String),
    ]);
    let (status, name) = (fixture.fields[0], fixture.fields[1]);
    let todo = fixture.option(status, "To do");
    let done = fixture.option(status, "Done");
    fixture.set(0, status, DatabaseValue::Enum(todo));
    fixture.set(0, name, DatabaseValue::String("Write it".into()));
    fixture.set(1, status, DatabaseValue::Enum(done));
    fixture.set(1, name, DatabaseValue::String("Ship it".into()));
    fixture.edit_view(DatabaseView::set_kanban_field(Some(status)));
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::Kanban));
    fixture.settle();

    let waiting = fixture.harness.rect_of("database-view.card.0");
    let finished = fixture.harness.rect_of("database-view.card.1");
    assert!(
        waiting.right() <= finished.left(),
        "the two cards share a column"
    );
    assert_eq!(fixture.harness.label("database-view.card.0"), "Write it");

    fixture.harness.click("database-view.card.1");
    fixture.settle();

    assert!(fixture.harness.shown("database-view.deselect"));
    fixture
        .harness
        .snapshot("cards_sit_in_the_kanban_column_their_status_names");
}
