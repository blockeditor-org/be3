use super::*;

#[test]
fn new_type_cells_render_and_a_boolean_cell_toggles() {
    let mut fixture = editor(&[
        ("Done", DatabaseFieldType::Boolean),
        ("Tint", DatabaseFieldType::Color),
        ("When", DatabaseFieldType::Datetime),
        ("Related", DatabaseFieldType::Block),
    ]);
    let boolean_id = fixture.fields[0];
    for (field_id, value) in [
        (fixture.fields[1], color_value()),
        (fixture.fields[2], DatabaseValue::Datetime(1_709_251_500)),
        (
            fixture.fields[3],
            DatabaseValue::Block(BlockRef::Direct(Uuid::from_u128(42))),
        ),
    ] {
        fixture.set(0, field_id, value);
    }
    fixture.run();

    fixture
        .harness
        .editor
        .click(&format!("database-view.cell.0.{boolean_id}"));
    fixture.run();
    fixture.run();

    assert_eq!(
        fixture.database().rows[0].value(boolean_id),
        Some(&DatabaseValue::Boolean(true))
    );
    fixture
        .harness
        .editor
        .snapshot("new_type_cells_render_and_a_boolean_cell_toggles");
}

fn color_value() -> DatabaseValue {
    DatabaseValue::Color(DatabaseColor {
        red: 0x10,
        green: 0x20,
        blue: 0x30,
        alpha: 0x40,
    })
}
