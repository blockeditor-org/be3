use block_editor_beui::beui::{PointerButton, Pos2, Vec2};
use database_types::database_schema::DatabaseFieldType;

use super::*;

#[test]
fn a_field_dragged_below_another_moves_after_it() {
    let mut harness = editor();
    let ids: Vec<Uuid> = ["A", "B", "C"]
        .into_iter()
        .map(|name| {
            let (id, edit) = DatabaseSchema::add_field(name, DatabaseFieldType::String);
            harness.edit::<DatabaseSchemaContent>(None, &edit);
            id
        })
        .collect();
    harness.run();
    let order = |harness: &BeuiTest<DatabaseSchemaApp>| {
        schema(harness)
            .fields()
            .iter()
            .map(|field| field.id)
            .collect::<Vec<Uuid>>()
    };
    let name = |id: Uuid| format!("database-schema.name.{id}");

    let field = harness.rect_of(&name(ids[0]));
    let grip = Pos2::new(field.left() - 12.0, field.center().y);
    let last = harness.rect_of(&name(ids[2]));
    let first = harness.rect_of(&name(ids[0]));
    let below = grip + Vec2::new(0.0, last.center().y - first.center().y + 12.0);
    harness.down(grip, PointerButton::Primary);
    harness.next_frame();
    harness.hover(grip + Vec2::new(0.0, 8.0));
    harness.next_frame();
    harness.hover(below);
    harness.next_frame();
    harness.up(below, PointerButton::Primary);
    harness.run();
    assert_eq!(order(&harness), vec![ids[1], ids[2], ids[0]]);

    harness.click(format!("database-schema.up.{}", ids[0]));
    harness.run();
    assert_eq!(
        order(&harness),
        vec![ids[1], ids[0], ids[2]],
        "Move up moves the field through the same list"
    );
}
