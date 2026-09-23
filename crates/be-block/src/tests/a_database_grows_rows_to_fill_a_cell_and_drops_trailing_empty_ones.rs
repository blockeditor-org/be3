use super::*;
use uuid::Uuid;

#[test]
fn a_database_grows_rows_to_fill_a_cell_and_drops_trailing_empty_ones() {
    let name = Uuid::new_v4();
    let text = |value: &str| Some(DatabaseValue::String(value.to_owned()));
    let mut database = DatabaseContent::default();
    let set = |database: &mut DatabaseContent, row: usize, value: Option<DatabaseValue>| {
        let edit = database.root().set_cell(row, name, value);
        database.apply(&edit);
    };

    set(&mut database, 0, text("first"));
    set(&mut database, 3, text("fourth"));
    let rows = database.root().rows;
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[3].value(name), text("fourth").as_ref());
    assert!(rows[1].values().is_empty());

    set(&mut database, 3, None);
    assert_eq!(database.root().rows.len(), 1);

    set(&mut database, 0, None);
    assert!(database.root().rows.is_empty());
}
