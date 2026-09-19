use super::*;

#[test]
fn cards_sit_in_the_kanban_column_their_status_names() {
    let mut fixture = editor(&[
        ("Status", DatabaseFieldType::Enum),
        ("Name", DatabaseFieldType::String),
    ]);
    let (status, name) = (fixture.fields[0], fixture.fields[1]);
    let (todo, done) = (Uuid::new_v4(), Uuid::new_v4());
    for (id, option) in [(todo, "To do"), (done, "Done")] {
        fixture
            .schema
            .operate(DatabaseSchemaOperation::AddEnumOption {
                field_id: status,
                option: DatabaseEnumOption {
                    id,
                    name: option.into(),
                },
            });
    }
    fixture.set(0, status, DatabaseValue::Enum(todo));
    fixture.set(0, name, DatabaseValue::String("Write it".into()));
    fixture.set(1, status, DatabaseValue::Enum(done));
    fixture.set(1, name, DatabaseValue::String("Ship it".into()));
    fixture.view.operate(DatabaseViewOperation::SetKanbanField {
        field_id: Some(status),
    });
    fixture.view.operate(DatabaseViewOperation::SetKind {
        kind: DatabaseViewKind::Kanban,
    });
    fixture.settle();

    let waiting = fixture.test.rect_of("database-view.card.0");
    let finished = fixture.test.rect_of("database-view.card.1");
    assert!(
        waiting.right() <= finished.left(),
        "the two cards share a column"
    );
    assert_eq!(fixture.test.label("database-view.card.0"), "Write it");

    fixture.test.click("database-view.card.1");
    fixture.settle();

    assert!(fixture.test.shown("database-view.deselect"));
    fixture
        .test
        .snapshot("cards_sit_in_the_kanban_column_their_status_names");
}
