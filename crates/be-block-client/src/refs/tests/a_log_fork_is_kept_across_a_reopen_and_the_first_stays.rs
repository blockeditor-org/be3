use uuid::Uuid;

use super::*;

/// The fork of a block's signed log a device found is its verdict for the block, kept in its
/// refs database across a restart; a fork found later leaves the first in place while it is
/// shown. Dismissing hides only that verdict: found again it stays hidden, but one of another
/// kind or against another head takes its place and shows.
#[test]
fn a_log_fork_is_kept_across_a_reopen_and_the_first_stays() {
    let path = std::env::temp_dir().join(format!("be-client-refs-{}.sqlite3", Uuid::new_v4()));
    let scope = Scope {
        server: "server".to_owned(),
        account: Uuid::new_v4(),
        organization: Uuid::new_v4(),
    };
    let block = Uuid::new_v4();
    let head = |size, byte| KeptLogHead {
        size,
        root: [byte; 32],
        frontier: None,
    };
    let first = LogFork {
        block,
        ours: head(3, 1),
        theirs: head(2, 2),
        prefix: Some([3; 32]),
        proof: vec![[6; 32]],
        from: HeadFrom::Session {
            account: scope.account,
            signature: vec![4; 64],
        },
        kind: ForkKind::Forked,
        found: Some(5),
        dismissed: false,
    };
    {
        let refs = RefsDb::open(&path).unwrap().scope(scope.clone()).unwrap();
        assert!(refs.found_fork(first.clone()).unwrap());
        let later = LogFork {
            from: HeadFrom::Server,
            kind: ForkKind::Misbehaved,
            ..first.clone()
        };
        assert!(!refs.found_fork(later).unwrap(), "the first fork stays");
    }

    let refs = RefsDb::open(&path).unwrap().scope(scope.clone()).unwrap();
    assert_eq!(
        refs.log_fork(block),
        Some(first.clone()),
        "the verdict outlives a restart"
    );
    assert!(refs.dismiss_fork(block).unwrap());
    assert!(!refs.dismiss_fork(block).unwrap(), "dismissed once");
    drop(refs);
    let refs = RefsDb::open(&path).unwrap().scope(scope).unwrap();
    assert_eq!(
        refs.log_fork(block),
        Some(LogFork {
            dismissed: true,
            ..first.clone()
        }),
        "dismissing keeps the verdict, marked so"
    );
    assert_eq!(refs.log_fork(Uuid::new_v4()), None);
    assert!(
        !refs
            .found_fork(LogFork {
                found: Some(9),
                ..first.clone()
            })
            .unwrap(),
        "the verdict dismissed, found again, stays hidden"
    );
    let misbehaved = LogFork {
        from: HeadFrom::Server,
        kind: ForkKind::Misbehaved,
        ..first.clone()
    };
    assert!(
        refs.found_fork(misbehaved.clone()).unwrap(),
        "another kind shows"
    );
    assert_eq!(refs.log_fork(block), Some(misbehaved.clone()));
    assert!(refs.dismiss_fork(block).unwrap());
    let other_head = LogFork {
        theirs: head(2, 7),
        ..misbehaved
    };
    assert!(
        refs.found_fork(other_head.clone()).unwrap(),
        "a verdict against another head shows"
    );
    assert_eq!(refs.log_fork(block), Some(other_head));

    drop(refs);
    let _ = std::fs::remove_file(&path);
}
