use uuid::Uuid;

use super::*;

/// The blocks the search indexer let go are this device's own state, kept in its refs database
/// so a restart reads them again rather than leaving their index entries stale.
#[test]
fn what_search_let_go_is_kept_across_a_reopen() {
    let path = std::env::temp_dir().join(format!("be-client-refs-{}.sqlite3", Uuid::new_v4()));
    let scope = Scope {
        server: "server".to_owned(),
        account: Uuid::new_v4(),
        organization: Uuid::new_v4(),
    };
    let (listed, unlisted, read) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let caught = ListedBlock {
        root: Uuid::new_v4(),
        content_type: Uuid::new_v4(),
        head: None,
    };
    {
        let refs = RefsDb::open(&path).unwrap().scope(scope.clone()).unwrap();
        refs.let_go(listed, Some(caught)).unwrap();
        refs.let_go(unlisted, None).unwrap();
        // Let go again with no listing, it keeps the one it had.
        refs.let_go(listed, None).unwrap();
        refs.let_go(read, None).unwrap();
        refs.forget_let_go(read).unwrap();
    }

    let refs = RefsDb::open(&path).unwrap().scope(scope).unwrap();
    let mut kept = refs.let_go_blocks();
    kept.sort_by_key(|(block, _)| *block);
    let mut expected = vec![(listed, Some(caught)), (unlisted, None)];
    expected.sort_by_key(|(block, _)| *block);
    assert_eq!(
        kept, expected,
        "what was let go is kept, and what was read is not"
    );

    drop(refs);
    let _ = std::fs::remove_file(&path);
}
