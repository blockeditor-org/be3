use super::*;

#[test]
fn adding_a_field_appends_a_string_field() {
    let mut harness = editor();

    harness.editor.click("database-schema.add-field");
    harness.run();

    let fields = schema(&harness).fields();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "Field");
    harness
        .editor
        .snapshot("adding_a_field_appends_a_string_field");
}
