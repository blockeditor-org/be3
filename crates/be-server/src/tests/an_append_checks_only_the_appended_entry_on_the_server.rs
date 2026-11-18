use be_block_keys::{
    Authority, Entry, EntryHash, Grant, Level, LogKind, Record, RecoveryPhrase, RecoverySecret,
};
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

/// The server keeps each log checked at its head: an append checks the appended entry alone, a
/// record that does not follow is refused without the log being checked again, and a log whose
/// stored entries are not what the server checked (another server's data) is checked whole.
#[test]
fn an_append_checks_only_the_appended_entry_on_the_server() {
    const LENGTH: u64 = 2_000;
    let creator = RecoveryPhrase::generate().secret();
    let (organization, block) = (Uuid::new_v4(), Uuid::new_v4());
    let root = Entry::root(&creator, [1; 16], LogKind::Block, block.into_bytes());
    let pinned = root.hash();
    let mut entries = vec![root.clone()];
    entries.extend(grants(&creator, block, pinned, 1, LENGTH - 1));
    let check = |entries: &[Entry]| {
        let before = be_block_keys::signatures_verified();
        let checked = crate::access::checked(
            LogKind::Block,
            organization,
            block,
            pinned,
            entries,
            &|_| None,
        );
        (checked, be_block_keys::signatures_verified() - before)
    };

    let (checked, first) = check(&entries);
    assert!(checked.is_ok());
    assert!(first >= LENGTH, "the first check verifies every entry");

    let appended = grants(&creator, block, entries.last().unwrap().hash(), LENGTH, 1);
    entries.extend(appended);
    let (checked, second) = check(&entries);
    assert_eq!(checked.unwrap().head, entries.last().unwrap().hash());
    assert_eq!(second, 1, "an append checks only the appended entry");

    let mut refused = entries.clone();
    let mut bad = grants(&creator, block, entries.last().unwrap().hash(), LENGTH + 1, 1).remove(0);
    bad.signature[0] ^= 1;
    refused.push(bad);
    let (checked, third) = check(&refused);
    assert!(checked.is_err(), "a record that does not follow is refused");
    assert!(third <= 1, "and checks no more than itself");

    // Another server's log under the same ids: its entries do not reach the kept head.
    let mut other = vec![root];
    other.extend(grants(&creator, block, pinned, LENGTH * 2, 10));
    let (checked, fourth) = check(&other);
    assert!(checked.is_ok());
    assert!(fourth >= 10, "a log that does not reach the kept head is checked whole");
}
