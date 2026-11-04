use super::*;

#[test]
fn a_changed_index_is_rebuilt_as_declared() {
    let connection = Connection::open_in_memory().unwrap();
    ensure(
        &connection,
        "CREATE TABLE tags (note TEXT NOT NULL, tag TEXT NOT NULL);
         CREATE INDEX tags_by_tag ON tags (tag);",
    )
    .unwrap();

    let unique = "CREATE TABLE tags (note TEXT NOT NULL, tag TEXT NOT NULL);
         CREATE UNIQUE INDEX tags_by_tag ON tags (note, tag);";
    ensure(&connection, unique).unwrap();
    ensure(&connection, unique).unwrap();

    connection
        .execute("INSERT INTO tags (note, tag) VALUES ('a', 'x')", [])
        .unwrap();
    assert!(
        connection
            .execute("INSERT INTO tags (note, tag) VALUES ('a', 'x')", [])
            .is_err(),
        "the index kept its old definition"
    );
}
