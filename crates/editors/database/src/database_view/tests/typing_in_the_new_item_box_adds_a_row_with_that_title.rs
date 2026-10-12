use super::*;

#[test]
fn typing_in_the_new_item_box_adds_a_row_with_that_title() {
    let mut fixture = editor(&[
        ("Title", DatabaseFieldType::String),
        ("Status", DatabaseFieldType::Enum),
    ]);
    let title = fixture.fields[0];
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::List));
    fixture.settle();

    fixture.harness.click("database-view.list.new");
    fixture.run();
    fixture.harness.text("Fix the login");
    fixture.run();
    fixture.harness.key_press(Key::Enter);
    fixture.settle();

    let database = fixture.database();
    assert_eq!(database.rows.len(), 1);
    let row = database.rows.get_index(0).unwrap();
    assert_eq!(
        row.value(title),
        Some(&DatabaseValue::String("Fix the login".into()))
    );
    assert!(
        fixture
            .harness
            .shown(&format!("database-view.list.row.{}", row.id.as_uuid()))
    );
}
