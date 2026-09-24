use super::*;

#[test]
fn selected_row_string_edit_updates_the_database() {
    let mut fixture = text_editor();
    let field_id = fixture.fields[0];

    fixture
        .harness
        .editor
        .click(&format!("database-view.cell.0.{field_id}"));
    fixture.run();
    fixture
        .harness
        .editor
        .click(&format!("database-view.selected-item.field.{field_id}"));
    fixture.harness.editor.text("alpha");
    fixture.run();
    fixture.run();

    assert_eq!(
        fixture.database().rows[0].value(field_id),
        Some(&DatabaseValue::String("alpha".to_owned()))
    );
    fixture
        .harness
        .editor
        .snapshot("selected_row_string_edit_updates_the_database");
}
