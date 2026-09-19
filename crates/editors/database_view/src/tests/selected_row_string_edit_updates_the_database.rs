use super::*;

#[test]
fn selected_row_string_edit_updates_the_database() {
    let mut fixture = text_editor();
    let field_id = fixture.fields[0];

    fixture
        .test
        .click(&format!("database-view.cell.0.{field_id}"));
    fixture.test.run();
    fixture
        .test
        .click(&format!("database-view.selected-item.field.{field_id}"));
    fixture.test.text("alpha");
    fixture.test.run();
    fixture.test.run();

    assert_eq!(
        fixture.database.read().unwrap().rows()[0].value(field_id),
        Some(&DatabaseValue::String("alpha".to_owned()))
    );
    fixture
        .test
        .snapshot("selected_row_string_edit_updates_the_database");
}
