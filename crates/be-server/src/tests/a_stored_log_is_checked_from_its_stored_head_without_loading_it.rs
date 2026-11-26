use be_block_keys::{
    Authority, Entry, EntryHash, Grant, Level, LogKind, Record, RecoveryPhrase, RecoverySecret,
};
use rusqlite::params;
use uuid::Uuid;

/// Grants of View to accounts never named before, each after the one before, from `previous`.
fn grants(
    creator: &RecoverySecret,
    block: Uuid,
    mut previous: EntryHash,
    from: u64,
    count: u64,
) -> Vec<Entry> {
    (from..from + count)
        .map(|index| {
            let mut grantee = [0; 16];
            grantee[..8].copy_from_slice(&index.to_be_bytes());
            let entry = Entry::sign(
                creator,
                [1; 16],
                block.into_bytes(),
                Some(previous),
                false,
                Record::Grant(Grant {
                    grantee,
                    identity: creator.identity(),
                    level: Level::View,
                    authority: Authority::Log,
                }),
            );
            previous = entry.hash();
            entry
        })
        .collect()
}

/// The server checks a stored log from the head it kept checked: once that head is the one
/// stored, a read loads and hashes only the log's first and last rows, and an append checks
/// and hashes only the appended entry besides, however long the log. Neither loads the log.
#[test]
fn a_stored_log_is_checked_from_its_stored_head_without_loading_it() {
    const LENGTH: u64 = 1_000;
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    crate::schema::initialize(&connection).unwrap();
    // The log's rows alone, with no block row they would hang from.
    connection
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    let creator = RecoveryPhrase::generate().secret();
    let (organization, block) = (Uuid::new_v4(), Uuid::new_v4());
    let root = Entry::root(&creator, [1; 16], LogKind::Block, block.into_bytes());
    let mut entries = vec![root.clone()];
    entries.extend(grants(&creator, block, root.hash(), 1, LENGTH - 1));
    let store = |position: usize, entry: &Entry| {
        connection
            .execute(
                "INSERT INTO access_log (organization_id, block_id, position, entry)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    organization.to_string(),
                    block.to_string(),
                    position,
                    entry.encode()
                ],
            )
            .unwrap();
    };
    for (position, entry) in entries.iter().enumerate() {
        store(position, entry);
    }
    let check = |appended: Option<&Entry>| {
        let (verified, hashed) = (
            be_block_keys::signatures_verified(),
            be_block_keys::entries_hashed(),
        );
        let checked =
            crate::access::checked_block_log(&connection, organization, block, appended).ok();
        (
            checked.flatten(),
            be_block_keys::signatures_verified() - verified,
            be_block_keys::entries_hashed() - hashed,
        )
    };

    let (checked, verified, _) = check(None);
    assert!(checked.is_some());
    assert!(verified >= LENGTH, "the first check verifies every entry");

    let (checked, verified, hashed) = check(None);
    assert_eq!(checked.unwrap().head, entries.last().unwrap().hash());
    assert_eq!(verified, 0, "a read verifies nothing");
    assert!(hashed <= 2, "and hashes only the first and last rows: {hashed}");

    let appended = grants(&creator, block, entries.last().unwrap().hash(), LENGTH, 1).remove(0);
    let (checked, verified, hashed) = check(Some(&appended));
    assert_eq!(checked.unwrap().head, appended.hash());
    assert_eq!(verified, 1, "an append verifies the appended entry alone");
    assert!(
        hashed <= 4,
        "and hashes it and the stored head's rows alone: {hashed}"
    );
    store(LENGTH as usize, &appended);

    let (checked, verified, hashed) = check(None);
    assert_eq!(checked.unwrap().head, appended.hash());
    assert_eq!(verified, 0);
    assert!(hashed <= 2, "{hashed}");
}
