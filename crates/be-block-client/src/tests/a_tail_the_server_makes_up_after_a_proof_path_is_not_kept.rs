use super::*;

/// A proof path's reduced entries after its last whole one could be a tail the server made up,
/// which no honest log reaches (guides/blocks/keys.md, "Proof paths"): a device keeps a path only
/// as far as its last whole entry, so the honest log served after it is taken, not withheld, while
/// one cut short of that whole entry still is.
#[test]
fn a_tail_the_server_makes_up_after_a_proof_path_is_not_kept() {
    let creator = RecoveryPhrase::generate().secret();
    let reader = RecoveryPhrase::generate().secret();
    let other = RecoveryPhrase::generate().secret();
    let (creator_id, reader_id) = (Uuid::new_v4(), Uuid::new_v4());
    let block = Uuid::new_v4();
    let root = Entry::root(
        &creator,
        creator_id.into_bytes(),
        LogKind::Block,
        block.into_bytes(),
    );
    let check = |steps: &[Step]| {
        be_block_keys::check_log(
            LogKind::Block,
            block.into_bytes(),
            root.hash(),
            steps,
            &|_| None,
        )
        .unwrap()
    };
    let grant = |steps: &[Step], grantee: Uuid, identity| -> Step {
        check(steps)
            .next(
                &creator,
                creator_id.into_bytes(),
                block.into_bytes(),
                Record::Grant(Grant {
                    grantee: grantee.into_bytes(),
                    identity,
                    level: Level::View,
                    authority: Authority::Log,
                }),
            )
            .unwrap()
            .into()
    };
    let mut log: Vec<Step> = vec![root.clone().into()];
    log.push(grant(&log, reader_id, reader.identity()));
    let mut honest = log.clone();
    honest.push(grant(&honest, Uuid::new_v4(), other.identity()));
    honest.push(grant(&honest, Uuid::new_v4(), other.identity()));
    let reduce = |steps: &[Step]| -> Vec<Step> {
        steps
            .iter()
            .enumerate()
            .map(|(at, step)| match (at, step) {
                (0 | 1, step) => step.clone(),
                (_, Step::Entry(entry)) => Step::Reduced(entry.reduced()),
                (_, reduced) => reduced.clone(),
            })
            .collect()
    };
    let honest = reduce(&honest);
    let made_up = [
        honest[..3].to_vec(),
        vec![Step::Reduced(Reduced {
            previous: Some(honest[2].hash()),
            lowers: false,
            places: false,
            owns: false,
            digest: [9; 32],
        })],
    ]
    .concat();

    let book = crate::KeyBook::new(reader_id, Uuid::new_v4(), reader.clone(), Keyring::new());
    book.pin_root(block, root.hash());
    let kept = book
        .note_log(block, &check(&made_up), &made_up)
        .expect("a first path is kept");
    assert_eq!(
        kept.head,
        log[1].hash(),
        "kept as far as its last whole entry"
    );

    let restarted = crate::KeyBook::new(reader_id, Uuid::new_v4(), reader, Keyring::new());
    restarted.set_kept_logs([(block, kept)]);
    assert!(
        matches!(
            restarted.served(block, &check(&honest)),
            crate::keys::Served::Take
        ),
        "the honest log after a made-up tail is withheld"
    );
    assert!(
        matches!(
            restarted.served(block, &check(&honest[..1])),
            crate::keys::Served::Withheld
        ),
        "a log cut short of the last whole entry kept is taken"
    );
}
