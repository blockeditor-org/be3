use super::*;
use block_editor_beui::beui::Modifiers;

#[test]
fn new_starts_a_row_and_typing_names_it() {
    let mut fixture = editor(&[
        ("Title", DatabaseFieldType::String),
        ("Status", DatabaseFieldType::Enum),
    ]);
    let title = fixture.fields[0];
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::List));
    fixture.settle();

    fixture.harness.click("database-view.new");
    fixture.harness.type_text("Fix the login");
    fixture.run();
    fixture.harness.key(Key::Enter, Modifiers::NONE);

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
