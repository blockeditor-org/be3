use super::*;

#[test]
fn arrow_keys_walk_the_spreadsheet_and_enter_steps_down() {
    let mut fixture = editor(&[
        ("Name", DatabaseFieldType::String),
        ("Note", DatabaseFieldType::String),
    ]);
    let first = fixture.fields[0];
    fixture.set(0, first, DatabaseValue::String("alpha".to_owned()));
    fixture.set(1, first, DatabaseValue::String("beta".to_owned()));
    fixture.settle();

    fixture
        .harness
        .editor
        .click(&format!("database-view.cell.0.{first}"));
    fixture.settle();

    assert_eq!(
        fixture
            .harness
            .editor
            .label("database-view.cell-editor.cell"),
        "A1"
    );

    fixture.harness.editor.key_press(Key::ArrowDown);
    fixture.settle();

    assert_eq!(
        fixture
            .harness
            .editor
            .label("database-view.cell-editor.cell"),
        "A2",
        "the down arrow must move the selection rather than scroll"
    );

    fixture.harness.editor.key_press(Key::ArrowRight);
    fixture.settle();

    assert_eq!(
        fixture
            .harness
            .editor
            .label("database-view.cell-editor.cell"),
        "B2"
    );

    fixture.harness.editor.click(&format!(
        "database-view.cell-editor.field.{}",
        fixture.fields[1]
    ));
    fixture.settle();
    fixture.harness.editor.key_press(Key::Enter);
    fixture.settle();

    assert_eq!(
        fixture
            .harness
            .editor
            .label("database-view.cell-editor.cell"),
        "B3",
        "enter in the cell editor must step down a row"
    );
}
