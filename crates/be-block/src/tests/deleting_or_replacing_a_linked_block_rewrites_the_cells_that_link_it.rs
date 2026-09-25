use super::*;
use crate::ChildChange;
use uuid::Uuid;

#[test]
fn deleting_or_replacing_a_linked_block_rewrites_the_cells_that_link_it() {
    let (field, other) = (Uuid::new_v4(), Uuid::new_v4());
    let (gone, old, new) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let link = |block| Some(DatabaseValue::Block(block));
    let mut database = DatabaseContent::default();
    for (row, field, value) in [
        (0, field, link(gone)),
        (0, other, Some(DatabaseValue::Number(2.0))),
        (1, field, link(old)),
    ] {
        let edit = database.root().set_cell(row, field, value);
        database.apply(&edit);
    }

    for change in [ChildChange::Delete(gone), ChildChange::Replace { old, new }] {
        let operations = database
            .child_operations(change)
            .expect("a database takes child changes");
        for operation in operations {
            database.apply(&operation);
        }
    }

    let rows = database.root().rows;
    assert_eq!(rows[0].value(field), None);
    assert_eq!(rows[0].value(other), Some(&DatabaseValue::Number(2.0)));
    assert_eq!(rows[1].value(field), link(new).as_ref());
}
