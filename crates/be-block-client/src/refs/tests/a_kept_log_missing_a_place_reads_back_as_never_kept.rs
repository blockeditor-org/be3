use be_block_keys::{LogKind, RecoveryPhrase, Step, check_log};
use uuid::Uuid;

use super::*;

/// A kept access log whose places do not all read back (a write lost between two keeps, or a
/// row gone) is not joined from what is left: it reads back as never kept, so the device checks
/// it afresh rather than taking part of a log for one it checked.
#[test]
fn a_kept_log_missing_a_place_reads_back_as_never_kept() {
    let path = std::env::temp_dir().join(format!("be-client-refs-{}.sqlite3", Uuid::new_v4()));
    let scope = Scope {
        server: "server".to_owned(),
        account: Uuid::new_v4(),
        organization: Uuid::new_v4(),
    };
    let (block, other) = (Uuid::new_v4(), Uuid::new_v4());
    let creator = RecoveryPhrase::generate().secret();
    let kept = |block: Uuid| {
        let root = be_block_keys::Entry::root(&creator, [1; 16], LogKind::Block, block.into_bytes());
        let steps = vec![Step::from(root.clone())];
        let checked =
            check_log(LogKind::Block, block.into_bytes(), root.hash(), &steps, &|_| None).unwrap();
        KeptLog {
            head: root.hash(),
            place: 0,
            lowering_place: 0,
            current: vec![root.hash()],
            levels: vec![([1; 16], be_block_keys::Level::Manage)],
            vouched: Vec::new(),
            checked: Some(checked),
            steps,
        }
    };
    {
        let refs = RefsDb::open(&path).unwrap().scope(scope.clone()).unwrap();
        refs.keep_log(block, kept(block)).unwrap();
        refs.keep_log(other, kept(other)).unwrap();
        // The root's place lost, as a write that never landed leaves it.
        let mut key = block.as_bytes().to_vec();
        key.extend(0u64.to_be_bytes());
        refs.write(vec![Row {
            table: Table::LogPlace,
            key,
            value: None,
        }])
        .unwrap();
    }
    let refs = RefsDb::open(&path).unwrap().scope(scope).unwrap();
    assert_eq!(
        refs.kept_logs(),
        vec![(other, kept(other))],
        "the log missing a place reads back as never kept, and the whole one as kept"
    );
    drop(refs);
    let _ = std::fs::remove_file(&path);
}
