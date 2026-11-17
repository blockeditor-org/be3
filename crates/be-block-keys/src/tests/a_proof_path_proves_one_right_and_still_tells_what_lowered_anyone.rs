use super::*;

/// A proof path (plans/signed_access.md, 3e) checks from the same root to the same head as the
/// whole log, with only the reader's grant and its granter's whole: no other member's grant
/// reaches it, yet it tells which epochs a revocation outdates, as the whole log does, and a
/// closing it cannot see shuts the kind it opened. An entry that says it lowered no one when it
/// did, or a reduced entry relabelled so, does not check.
#[test]
fn a_proof_path_proves_one_right_and_still_tells_what_lowered_anyone() {
    let creator = RecoveryPhrase::generate().secret();
    let manager = RecoveryPhrase::generate().secret();
    let editor = RecoveryPhrase::generate().secret();
    let reader = RecoveryPhrase::generate().secret();
    let (creator_id, manager_id, editor_id, reader_id) = ([1; 16], [2; 16], [3; 16], [4; 16]);
    let block = [7; 16];
    let none = |_: &Asked| None;
    let root = Entry::root(&creator, creator_id, LogKind::Block, block);
    let mut log = vec![root.clone()];
    let append = |log: &mut Vec<Entry>, by: &RecoverySecret, signer, record| {
        let checked = check_log(LogKind::Block, block, root.hash(), log, &none).unwrap();
        log.push(checked.next(by, signer, block, record).unwrap());
    };
    let grant = |grantee, identity, level| {
        Record::Grant(Grant {
            grantee,
            identity,
            level,
            authority: Authority::Log,
        })
    };
    append(
        &mut log,
        &creator,
        creator_id,
        grant(manager_id, manager.identity(), Level::Manage),
    );
    append(
        &mut log,
        &creator,
        creator_id,
        grant(editor_id, editor.identity(), Level::Edit),
    );
    append(
        &mut log,
        &creator,
        creator_id,
        Record::Opening(Opening {
            level: Level::View,
            authority: Authority::Log,
        }),
    );
    let opened = log.last().unwrap().hash();
    append(
        &mut log,
        &manager,
        manager_id,
        grant(reader_id, reader.identity(), Level::View),
    );
    let granted = log.last().unwrap().hash();
    append(
        &mut log,
        &creator,
        creator_id,
        grant(editor_id, editor.identity(), Level::None),
    );
    assert!(log.last().unwrap().lowers, "a revocation says it lowers");

    let whole = check_log(LogKind::Block, block, root.hash(), &log, &none).unwrap();
    let path = proof_path(LogKind::Block, &log, reader_id, []);
    let checked = check_log(LogKind::Block, block, root.hash(), &path, &none).unwrap();
    assert!(checked.partial && !whole.partial && checked.hides_lowering);
    assert_eq!(checked.head, whole.head, "the path reaches the same head");
    assert_eq!(checked.positions, whole.positions);
    assert_eq!(
        checked.level_as_of(reader_id, &checked.head),
        Some(Level::View)
    );
    assert_eq!(
        checked.level_as_of(manager_id, &granted),
        Some(Level::Manage),
        "the granter's own right is on the path"
    );
    assert!(
        !checked.grants.contains_key(&editor_id),
        "no other member's grant reaches the reader"
    );
    assert!(
        path.iter().filter_map(Step::entry).all(
            |entry| !matches!(&entry.record, Record::Grant(grant) if grant.grantee == editor_id)
        ),
    );
    assert!(
        checked.outdates(&opened) && whole.outdates(&opened),
        "the reduced revocation outdates what came before it"
    );
    assert!(checked.current_at(&checked.head));
    assert_eq!(whole.open_inside, Level::View);
    assert_eq!(
        checked.open_inside,
        Level::None,
        "a reduced lowering after an opening may have closed it"
    );
    assert!(
        checked
            .next(
                &manager,
                manager_id,
                block,
                grant(editor_id, editor.identity(), Level::View)
            )
            .is_none(),
        "nothing but the reader's own grant is signed on a path"
    );
    assert!(
        checked
            .next(
                &reader,
                reader_id,
                block,
                grant(reader_id, reader.identity(), Level::None)
            )
            .is_some_and(|leaving| leaving.lowers),
        "a reader leaves, and says so"
    );

    // The revocation says it lowered no one: refused, whole or reduced.
    let mut quiet = log.clone();
    let last = quiet.pop().unwrap();
    quiet.push(Entry::sign(
        &creator,
        creator_id,
        block,
        last.previous,
        false,
        last.record.clone(),
    ));
    assert!(check_log(LogKind::Block, block, root.hash(), &quiet, &none).is_err());
    let mut relabelled = path.clone();
    if let Some(Step::Reduced(reduced)) = relabelled.last_mut() {
        reduced.lowers = false;
    }
    let relabelled = check_log(LogKind::Block, block, root.hash(), &relabelled, &none).unwrap();
    assert!(
        !relabelled.holds(&whole.head),
        "a relabelled entry is another entry, so the path no longer reaches the head"
    );
    for entry in path.iter().filter_map(Step::entry) {
        assert_eq!(Step::Reduced(entry.reduced()).hash(), entry.hash());
    }
}
