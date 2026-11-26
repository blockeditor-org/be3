use be_block_store::{Ledger, Noted};
use uuid::Uuid;

use super::*;

/// Two scopes' part writers race to note one object: the ledger keeps the first's note and
/// answers it to the second, across a reopen too, so a crash never leaves the object noted as
/// the second owner's while the first's commit may name it.
#[test]
fn a_ledger_keeps_an_objects_first_note() {
    let path = std::env::temp_dir().join(format!("be-client-ledger-{}.sqlite3", Uuid::new_v4()));
    let object = Hash::of(b"object");
    let block = Uuid::new_v4();
    let first = Noted {
        block,
        owner: Hash::of(b"first"),
    };
    let second = Noted {
        block,
        owner: Hash::of(b"second"),
    };
    let ledger = |db: &RefsDb| DeviceLedger {
        backend: Arc::clone(&db.backend),
        store: b"store".to_vec(),
    };
    {
        let db = RefsDb::open(&path).unwrap();
        let ledger = ledger(&db);
        assert_eq!(ledger.note(object, first).unwrap(), None, "noted");
        assert_eq!(ledger.note(object, second).unwrap(), Some(first), "kept");
        assert_eq!(ledger.note(object, first).unwrap(), Some(first), "again");
    }

    let db = RefsDb::open(&path).unwrap();
    assert_eq!(ledger(&db).noted().unwrap(), vec![(object, first)]);
    ledger(&db).forget(&[object]).unwrap();
    assert_eq!(
        ledger(&db).note(object, second).unwrap(),
        None,
        "noted anew"
    );

    drop(db);
    let _ = std::fs::remove_file(&path);
}
