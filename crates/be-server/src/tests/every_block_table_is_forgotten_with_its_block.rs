/// `forget_blocks` deletes a block's rows table by table: a table with a `block_id` column that
/// neither list names would keep rows of a forgotten block.
#[test]
fn every_block_table_is_forgotten_with_its_block() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    crate::schema::initialize(&connection).unwrap();
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND EXISTS
                 (SELECT 1 FROM pragma_table_info(sqlite_master.name) WHERE name = 'block_id')",
        )
        .unwrap();
    let tables: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        tables.iter().any(|table| table == "share_links"),
        "{tables:?}"
    );
    let listed = |table: &str| {
        crate::blocks::BLOCK_ROWS.contains(&table)
            || crate::blocks::BLOCK_ROWS_APART.contains(&table)
    };
    let missing: Vec<&String> = tables.iter().filter(|table| !listed(table)).collect();
    assert!(
        missing.is_empty(),
        "{missing:?} keep rows by block_id: list each in BLOCK_ROWS, which forget_blocks deletes, \
         or in BLOCK_ROWS_APART with how forget_blocks handles it"
    );
    let named = crate::blocks::BLOCK_ROWS
        .iter()
        .chain(crate::blocks::BLOCK_ROWS_APART);
    let unknown: Vec<&&str> = named
        .filter(|table| !tables.iter().any(|known| known == **table))
        .collect();
    assert!(
        unknown.is_empty(),
        "{unknown:?} are listed but have no block_id column"
    );
}
