use super::*;

#[test]
fn a_list_shows_each_row_by_its_short_id_and_opens_it_beside_the_list() {
    let mut fixture = editor(&[
        ("Title", DatabaseFieldType::String),
        ("Status", DatabaseFieldType::Enum),
    ]);
    let (title, status) = (fixture.fields[0], fixture.fields[1]);
    let todo = fixture.option(status, "To do");
    let done = fixture.option(status, "Done");
    fixture.set(0, title, DatabaseValue::String("Write the guide".into()));
    fixture.set(0, status, DatabaseValue::Enum(done));
    fixture.set(1, title, DatabaseValue::String("Ship the tracker".into()));
    fixture.set(1, status, DatabaseValue::Enum(todo));
    let database = fixture.database;
    fixture
        .harness
        .edit::<DatabaseContent>(Some(database), &Database::set_prefix("BE"));
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::List));
    fixture.settle();

    let rows = fixture.database();
    let second = rows.rows.get_index(1).unwrap().id;
    let line = format!("database-view.list.row.{}", second.as_uuid());
    assert!(fixture.harness.shown(&line));
    assert!(!fixture.harness.shown("database-view.detail.title"));

    fixture.harness.click(&line);

    assert_eq!(
        fixture.harness.label("database-view.detail.id"),
        rows.short_id(second)
    );
    assert!(rows.short_id(second).starts_with("BE-"));
    assert!(
        fixture
            .harness
            .label("database-view.detail.title")
            .starts_with("Ship the tracker")
    );
    fixture
        .harness
        .snapshot("a_list_shows_each_row_by_its_short_id_and_opens_it_beside_the_list");
}
