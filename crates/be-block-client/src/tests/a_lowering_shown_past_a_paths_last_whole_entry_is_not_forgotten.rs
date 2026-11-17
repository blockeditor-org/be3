use super::*;

/// A lowering a proof path showed in its reduced tail, past its last whole entry, stands once
/// shown: a later path that leaves the tail out outdates the same epochs, kept and in this run.
#[test]
fn a_lowering_shown_past_a_paths_last_whole_entry_is_not_forgotten() {
    let creator = RecoveryPhrase::generate().secret();
    let reader = RecoveryPhrase::generate().secret();
    let (creator_id, reader_id, other_id) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
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
            &|_, _| None,
        )
        .unwrap()
    };
    let grant = |steps: &[Step], grantee: Uuid, level| -> Step {
        check(steps)
            .next(
                &creator,
                creator_id.into_bytes(),
                block.into_bytes(),
                Record::Grant(Grant {
                    grantee: grantee.into_bytes(),
                    identity: reader.identity(),
                    level,
                    authority: Authority::Log,
                }),
            )
            .unwrap()
            .into()
    };
    let mut whole: Vec<Step> = vec![root.clone().into()];
    whole.push(grant(&whole, other_id, Level::Edit));
    whole.push(grant(&whole, reader_id, Level::View));
    let granted = whole[2].hash();
    whole.push(grant(&whole, other_id, Level::None));
    let path: Vec<Step> = whole
        .iter()
        .enumerate()
        .map(|(at, step)| match (at, step) {
            (1 | 3, Step::Entry(entry)) => Step::Reduced(entry.reduced()),
            (_, step) => step.clone(),
        })
        .collect();

    let book = crate::KeyBook::new(reader_id, Uuid::new_v4(), reader, Keyring::new());
    let kept = book.note_log(block, &check(&path), &path).unwrap();
    assert_eq!(kept.lowering_place, 3);
    assert!(kept.current.is_empty());
    let dropped = &path[..3];
    let again = book
        .note_log(block, &check(dropped), dropped)
        .unwrap_or(kept);
    assert_eq!(
        again.lowering_place, 3,
        "a path that drops the tail forgets the lowering it held"
    );
    assert!(!again.current.contains(&granted));
    assert!(
        matches!(
            book.served(block, &check(dropped)),
            crate::keys::Served::Take
        ),
        "the shorter path is withheld"
    );
}
