use super::*;

#[test]
fn a_column_that_changed_or_cannot_be_added_is_a_mismatch() {
    let connection = Connection::open_in_memory().unwrap();
    ensure(&connection, FIRST).unwrap();

    let changed = "
        CREATE TABLE notes (id TEXT PRIMARY KEY, body BLOB NOT NULL, owner TEXT NOT NULL);
        CREATE TABLE tags (note TEXT NOT NULL);
    ";
    let Err(SchemaError::Mismatch(problems)) = ensure(&connection, changed) else {
        panic!("a changed schema was accepted");
    };
    assert_eq!(
        problems,
        [
            "notes.body has a different type",
            "notes.owner is missing and cannot be added to an existing table",
        ]
    );
    let tables: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'tags'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tables, 0, "a refused schema changed the database");

    let Err(SchemaError::Mismatch(problems)) =
        ensure(&connection, "CREATE TABLE notes (id TEXT PRIMARY KEY);")
    else {
        panic!("a column this build does not know was accepted");
    };
    assert_eq!(problems, ["notes.body is not a column this build knows"]);
}
