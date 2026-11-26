use super::*;

/// A grant resting on authority in another log checks that log as of the head it names once:
/// a later check of the block's log, through other `Logs` keeping the same `CheckedAsOf`,
/// verifies only the block's own entries, as the log above was kept checked as of that head.
#[test]
fn authority_from_another_log_is_checked_once_as_of_each_head() {
    const ABOVE: usize = 200;
    let alice = RecoveryPhrase::generate().secret();
    let manager = RecoveryPhrase::generate().secret();
    let (alice_id, manager_id) = ([1; 16], [2; 16]);
    let (top, below) = ([0xa5; 16], [0xa6; 16]);

    // The top: many grants, then Alice grants the manager Manage.
    let top_root = Entry::root(&alice, alice_id, LogKind::Block, top);
    let mut top_log = vec![top_root];
    for index in 0..ABOVE {
        let mut grantee = [0; 16];
        grantee[..8].copy_from_slice(&(index as u64 + 1000).to_be_bytes());
        let next = Entry::sign(
            &alice,
            alice_id,
            top,
            Some(top_log.last().unwrap().hash()),
            false,
            Record::Grant(Grant {
                grantee,
                identity: RecoveryPhrase::generate().secret().identity(),
                level: Level::View,
                authority: Authority::Log,
            }),
        );
        top_log.push(next);
    }
    let manages = Entry::sign(
        &alice,
        alice_id,
        top,
        Some(top_log.last().unwrap().hash()),
        false,
        Record::Grant(Grant {
            grantee: manager_id,
            identity: manager.identity(),
            level: Level::Manage,
            authority: Authority::Log,
        }),
    );
    top_log.push(manages);

    // Below it, a grant the manager signs on authority from the top as of its head.
    let root = Entry::root(&alice, alice_id, LogKind::Block, below);
    let placed = Entry::sign(
        &alice,
        alice_id,
        below,
        Some(root.hash()),
        false,
        Record::Parent(Parent {
            placement: Placement::Block {
                block: top,
                root: top_log[0].hash(),
            },
            authority: Authority::Log,
        }),
    );
    let granted = Entry::sign(
        &manager,
        manager_id,
        below,
        Some(placed.hash()),
        false,
        Record::Grant(Grant {
            grantee: [9; 16],
            identity: RecoveryPhrase::generate().secret().identity(),
            level: Level::View,
            authority: Authority::Inherited(vec![Link {
                block: top,
                head: top_log.last().unwrap().hash(),
            }]),
        }),
    );
    let below_log = vec![root, placed, granted];

    let log = |_: LogKind, of: [u8; 16]| {
        (of == top).then(|| top_log.iter().cloned().map(Step::from).collect())
    };
    let kept = CheckedAsOf::new(1 << 16);
    let check = || {
        let held = Logs::new([0; 16], None, &log).keeping(&kept);
        let outside = |asked: &Asked| answer(&held, below, asked);
        let before = signatures_verified();
        let checked = check_log(
            LogKind::Block,
            below,
            below_log[0].hash(),
            &below_log,
            &outside,
        )
        .expect("a grant resting on the top checks");
        assert!(checked.grants.contains_key(&[9; 16]));
        (signatures_verified() - before, held.checks())
    };

    let (first, checks) = check();
    assert!(
        first > ABOVE as u64,
        "the first check verifies the log above: {first}"
    );
    assert_eq!(checks, 1);
    let (second, checks) = check();
    assert_eq!(
        second,
        below_log.len() as u64,
        "a later check verifies only the block's own entries"
    );
    assert_eq!(checks, 0, "and checks no log above again");
}
