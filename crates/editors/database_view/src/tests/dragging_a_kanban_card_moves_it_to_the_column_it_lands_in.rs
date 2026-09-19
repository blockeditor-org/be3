use super::*;

#[test]
fn dragging_a_kanban_card_moves_it_to_the_column_it_lands_in() {
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
    fixture.test.drag(waiting.center(), finished.center());
    fixture.settle();

    assert_eq!(
        fixture.database.read().unwrap().rows()[0].value(status),
        Some(&DatabaseValue::Enum(done)),
        "the card must take the status of the column it was dropped in"
    );

    let moved = fixture.test.rect_of("database-view.card.0");
    assert!(
        moved.left() >= waiting.right(),
        "the card must now sit in the column it was dropped in"
    );
}
