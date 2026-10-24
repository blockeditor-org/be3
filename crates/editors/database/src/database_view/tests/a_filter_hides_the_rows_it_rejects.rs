use database_types::database_view::FilterCondition;

use super::*;

#[test]
fn a_filter_hides_the_rows_it_rejects() {
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
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::List));
    let (_, filter) = DatabaseView::add_filter(
        Some(status),
        FilterCondition::IsNot(DatabaseValue::Enum(done)),
    );
    fixture.edit_view(filter);
    fixture.settle();

    let rows = fixture.database();
    let line = |index: usize| {
        format!(
            "database-view.list.row.{}",
            rows.rows.get_index(index).unwrap().id.as_uuid()
        )
    };
    assert!(!fixture.harness.shown(&line(0)));
    assert!(fixture.harness.shown(&line(1)));
    fixture
        .harness
        .snapshot("a_filter_hides_the_rows_it_rejects");
}
