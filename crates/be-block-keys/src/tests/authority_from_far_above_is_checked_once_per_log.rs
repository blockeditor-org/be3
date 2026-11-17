use super::*;

/// Authority resting on authority from far above costs one check of each log as of each head
/// (`Logs`), not one per path to it: a chain of blocks, each with a grant resting on the top
/// through every block between, checks in as many log checks as there are blocks.
#[test]
fn authority_from_far_above_is_checked_once_per_log() {
    const DEPTH: usize = 24;
    let alice = RecoveryPhrase::generate().secret();
    let manager = RecoveryPhrase::generate().secret();
    let (alice_id, manager_id) = ([1; 16], [2; 16]);
    let block = |at: usize| [10 + at as u8; 16];

    // The top, where Alice grants the manager Manage.
    let top_root = Entry::root(&alice, alice_id, LogKind::Block, block(0));
    let manages = Entry::sign(
        &alice,
        alice_id,
        block(0),
        Some(top_root.hash()),
        false,
        Record::Grant(Grant {
            grantee: manager_id,
            identity: manager.identity(),
            level: Level::Manage,
            authority: Authority::Log,
        }),
    );
    let mut logs: Vec<([u8; 16], Vec<Entry>)> = vec![(block(0), vec![top_root, manages])];
    // Each block below, placed under the one above, with a grant the manager signs resting on
    // the top through every block between.
    for at in 1..=DEPTH {
        let root = Entry::root(&alice, alice_id, LogKind::Block, block(at));
        let above = &logs[at - 1].1;
        let placed = Entry::sign(
            &alice,
            alice_id,
            block(at),
            Some(root.hash()),
            false,
            Record::Parent(Parent {
                placement: Placement::Block {
                    block: block(at - 1),
                    root: above[0].hash(),
                },
                authority: Authority::Log,
            }),
        );
        let chain = (0..at)
            .rev()
            .map(|up| Link {
                block: block(up),
                head: logs[up].1.last().unwrap().hash(),
            })
            .collect();
        let grantee = [100 + at as u8; 16];
        let granted = Entry::sign(
            &manager,
            manager_id,
            block(at),
            Some(placed.hash()),
            false,
            Record::Grant(Grant {
                grantee,
                identity: RecoveryPhrase::generate().secret().identity(),
                level: Level::View,
                authority: Authority::Inherited(chain),
            }),
        );
        logs.push((block(at), vec![root, placed, granted]));
    }

    let log = |_: LogKind, of: [u8; 16]| {
        logs.iter()
            .find(|(id, _)| *id == of)
            .map(|(_, entries)| entries.iter().cloned().map(Step::from).collect())
    };
    let held = Logs::new([0; 16], None, &log);
    let (bottom, entries) = logs.last().unwrap();
    let outside = |asked: &Asked| answer(&held, *bottom, asked);
    let checked = check_log(LogKind::Block, *bottom, entries[0].hash(), entries, &outside)
        .expect("a grant resting on the top checks");
    assert!(checked.grants.contains_key(&[100 + DEPTH as u8; 16]));
    assert!(
        held.checks() <= DEPTH,
        "{} log checks for a chain {DEPTH} deep",
        held.checks()
    );
}
