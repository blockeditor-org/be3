use super::*;

#[test]
fn a_share_link_grants_whoever_opens_it_until_it_is_revoked() {
    let owner = RecoveryPhrase::generate().secret();
    let viewer = RecoveryPhrase::generate().secret();
    let opener = RecoveryPhrase::generate().secret();
    let (owner_id, viewer_id, opener_id) = ([1; 16], [2; 16], [3; 16]);
    let block = [7; 16];
    let none = |_: &Asked| None;
    let root = Entry::root(&owner, owner_id, LogKind::Block, block);
    let pinned = root.hash();
    let check = |entries: &[Entry]| check_log(LogKind::Block, block, pinned, entries, &none);
    let link = |by: &RecoverySecret, signer, previous, code: &InviteCode, open, lowers| {
        Entry::sign(
            by,
            signer,
            block,
            Some(previous),
            lowers,
            Record::ShareLink(ShareLink {
                key: code.share_key(block),
                level: Level::Edit,
                admits: Admits::Anyone,
                open,
                authority: Authority::Log,
            }),
        )
    };

    let code = InviteCode::generate();
    let made = link(&owner, owner_id, pinned, &code, true, false);
    let joined = Entry::opened(&code, block, made.hash(), opener_id, &opener, Level::Edit);
    let checked = check(&[root.clone(), made.clone(), joined.clone()]).unwrap();
    assert_eq!(
        checked.grants.get(&opener_id),
        Some(&(Level::Edit, opener.identity())),
        "the opener holds what the link grants, with the keys it signed with"
    );
    assert_eq!(checked.linked.get(&opener_id), Some(&code.share_key(block)));
    let path = proof_path(LogKind::Block, &[root.clone(), made.clone(), joined.clone()], opener_id, []);
    let partial = check_log(LogKind::Block, block, pinned, &path, &none).unwrap();
    assert_eq!(
        partial.level_as_of(opener_id, &joined.hash()),
        Some(Level::Edit),
        "an opener's path proves its grant through the link"
    );

    let other = InviteCode::generate();
    let forged = Entry::opened(&other, block, made.hash(), opener_id, &opener, Level::View);
    assert!(
        check(&[root.clone(), made.clone(), forged]).is_err(),
        "a code no manager made a link of grants nothing"
    );
    let more = Entry::opened(&code, block, made.hash(), opener_id, &opener, Level::Manage);
    assert!(
        check(&[root.clone(), made.clone(), more]).is_err(),
        "a link grants no more than its level"
    );
    let by_viewer = {
        let grant = Entry::sign(
            &owner,
            owner_id,
            block,
            Some(pinned),
            false,
            Record::Grant(Grant {
                grantee: viewer_id,
                identity: viewer.identity(),
                level: Level::View,
                authority: Authority::Log,
            }),
        );
        let made = link(&viewer, viewer_id, grant.hash(), &code, true, false);
        check(&[root.clone(), grant, made])
    };
    assert!(by_viewer.is_err(), "only a manager makes a link");
    let mut swapped = Entry::opened(&code, block, made.hash(), opener_id, &opener, Level::View);
    if let Record::Opened(opened) = &mut swapped.record {
        opened.identity = viewer.identity();
    }
    assert!(
        check(&[root.clone(), made.clone(), swapped]).is_err(),
        "a holder of the code names no keys it does not hold"
    );

    // A viewer the log names already opens a link only with the keys it names for them.
    let granted = Entry::sign(
        &owner,
        owner_id,
        block,
        Some(made.hash()),
        false,
        Record::Grant(Grant {
            grantee: viewer_id,
            identity: viewer.identity(),
            level: Level::View,
            authority: Authority::Log,
        }),
    );
    let impostor = RecoveryPhrase::generate().secret();
    let swapped_keys = Entry::opened(&code, block, granted.hash(), viewer_id, &impostor, Level::Edit);
    assert!(
        check(&[root.clone(), made.clone(), granted.clone(), swapped_keys]).is_err(),
        "a link's grant replaces no keys the log names for an account"
    );
    let raised = Entry::opened(&code, block, granted.hash(), viewer_id, &viewer, Level::Edit);
    assert!(
        check(&[root.clone(), made.clone(), granted.clone(), raised]).is_ok(),
        "the same keys raise them"
    );

    // A grantee who took a new phrase opens a link with its new keys, never its retired ones.
    let renewed = RecoveryPhrase::generate().secret();
    let retired = Entry::retirement(&viewer, &renewed, viewer_id, block, granted.hash());
    let with_new = Entry::opened(&code, block, retired.hash(), viewer_id, &renewed, Level::Edit);
    assert!(
        check(&[root.clone(), made.clone(), granted.clone(), retired.clone(), with_new]).is_ok(),
        "a grantee's new keys open a link after its retirement"
    );
    let with_old = Entry::opened(&code, block, retired.hash(), viewer_id, &viewer, Level::Edit);
    assert!(
        check(&[root.clone(), made.clone(), granted.clone(), retired, with_old]).is_err(),
        "its retired keys open nothing"
    );

    let revoked = link(&owner, owner_id, joined.hash(), &code, false, true);
    let checked = check(&[root.clone(), made.clone(), joined.clone(), revoked.clone()]).unwrap();
    assert_eq!(
        checked.grants.get(&opener_id).map(|(level, _)| *level),
        Some(Level::None),
        "revoking the link takes back every grant made through it"
    );
    assert!(checked.outdates(&joined.hash()), "and outdates the epochs before");
    let path = proof_path(
        LogKind::Block,
        &[root.clone(), made.clone(), joined.clone(), revoked.clone()],
        viewer_id,
        [],
    );
    let partial = check_log(LogKind::Block, block, pinned, &path, &none).unwrap();
    assert!(
        partial.outdates(&joined.hash()),
        "a path that hides whom a revoked link granted still outdates the epochs before"
    );
    let late = Entry::opened(&code, block, revoked.hash(), viewer_id, &viewer, Level::View);
    assert!(
        check(&[root.clone(), made, joined, revoked, late]).is_err(),
        "a revoked link grants no one"
    );
}
