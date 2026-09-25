use super::*;
use uuid::Uuid;

#[test]
fn a_database_and_its_views_reference_what_they_link_to() {
    let schema = Uuid::new_v4();
    let linked = Uuid::new_v4();
    let field = Uuid::new_v4();
    let other_field = Uuid::new_v4();
    let mut database = DatabaseContent::new(&Database::with_schema(schema));
    for (row, value) in [
        (0, DatabaseValue::Block(linked)),
        (1, DatabaseValue::Block(linked)),
    ] {
        let edit = database.root().set_cell(row, field, Some(value));
        database.apply(&edit);
    }
    let edit = database
        .root()
        .set_cell(0, other_field, Some(DatabaseValue::Number(1.0)));
    database.apply(&edit);

    assert_eq!(BlockContent::references(&database), [schema, linked]);

    let database_id = Uuid::new_v4();
    let view = DatabaseViewContent::new(&DatabaseView::of(database_id));
    assert_eq!(BlockContent::references(&view), [database_id]);
}
