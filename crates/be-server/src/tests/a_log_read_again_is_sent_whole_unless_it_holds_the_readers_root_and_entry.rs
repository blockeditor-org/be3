use be_block_keys::{Authority, Entry, Grant, Level, LogKind, Record, RecoveryPhrase};
use be_block_protocol::HeldEntry;
use rusqlite::params;
use uuid::Uuid;

/// A reader holding a log names its root and its last entry: the server sends only the
/// entries after it while it stores that entry at that place from that root, and the log from
/// its root otherwise (a root or an entry stored in another's place), for the reader to refuse.
#[test]
fn a_log_read_again_is_sent_whole_unless_it_holds_the_readers_root_and_entry() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    crate::schema::initialize(&connection).unwrap();
    connection
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    let creator = RecoveryPhrase::generate().secret();
    let (organization, block) = (Uuid::new_v4(), Uuid::new_v4());
    let mut entries = vec![Entry::root(&creator, [1; 16], LogKind::Block, block.into_bytes())];
    for index in 0..4u8 {
        let previous = entries.last().unwrap().hash();
        entries.push(Entry::sign(
            &creator,
            [1; 16],
            block.into_bytes(),
            Some(previous),
            false,
            Record::Grant(Grant {
                grantee: [10 + index; 16],
                identity: creator.identity(),
                level: Level::View,
                authority: Authority::Log,
            }),
        ));
    }
    for (position, entry) in entries.iter().enumerate() {
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
    }
    let read = |held: HeldEntry| {
        crate::access::served_log(
            &connection,
            LogKind::Block,
            organization,
            block,
            None,
            Vec::new(),
            Some(held),
        )
        .unwrap()
    };
    let held = HeldEntry {
        place: 2,
        hash: entries[2].hash(),
        root: entries[0].hash(),
    };

    let page = read(held.clone());
    assert_eq!(page.from, 3, "the entries after the one held");
    assert_eq!(page.entries.len(), 2);

    let stranger = RecoveryPhrase::generate().secret();
    let other_root = Entry::root(&stranger, [2; 16], LogKind::Block, block.into_bytes()).hash();
    let page = read(HeldEntry {
        root: other_root,
        ..held.clone()
    });
    assert_eq!(page.from, 0, "a reader holding another root is sent the log from its root");
    assert_eq!(page.entries.len(), entries.len());

    let page = read(HeldEntry {
        hash: entries[1].hash(),
        ..held
    });
    assert_eq!(page.from, 0, "an entry the log does not store there is sent from the root");
}
