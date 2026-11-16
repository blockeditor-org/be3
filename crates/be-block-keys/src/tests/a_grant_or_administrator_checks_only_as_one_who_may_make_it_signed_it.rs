use super::*;

#[test]
fn a_grant_or_administrator_checks_only_as_one_who_may_make_it_signed_it() {
    let creator = RecoveryPhrase::generate().secret();
    let member = RecoveryPhrase::generate().secret();
    let other = RecoveryPhrase::generate().secret();
    let (creator_id, member_id, other_id) = ([1; 16], [2; 16], [3; 16]);
    let block = [7; 16];
    let none = |_| None;

    let root = Entry::root(&creator, creator_id, LogKind::Block, block);
    let pinned = root.hash();
    let grant = |by: &RecoverySecret, signer, previous, grantee, identity, level| {
        Entry::sign(
            by,
            signer,
            block,
            Some(previous),
            Record::Grant(Grant {
                grantee,
                identity,
                level,
            }),
        )
    };
    let edit = grant(&creator, creator_id, pinned, member_id, member.identity(), Level::Edit);
    let checked = check_log(LogKind::Block, block, pinned, &[root.clone(), edit.clone()], &none)
        .unwrap();
    assert_eq!(
        checked.grants.get(&member_id),
        Some(&(Level::Edit, member.identity())),
        "the creator manages the block and grants it"
    );

    let onward = grant(&member, member_id, edit.hash(), other_id, other.identity(), Level::View);
    assert!(
        check_log(LogKind::Block, block, pinned, &[root.clone(), edit.clone(), onward], &none)
            .is_err(),
        "a member with Edit grants no one"
    );
    let own = grant(&member, member_id, edit.hash(), member_id, member.identity(), Level::None);
    assert!(
        check_log(LogKind::Block, block, pinned, &[root.clone(), edit.clone(), own], &none)
            .is_ok(),
        "anyone revokes their own grant"
    );
    let stranger = grant(&other, other_id, pinned, other_id, other.identity(), Level::Manage);
    assert!(
        check_log(LogKind::Block, block, pinned, &[root.clone(), stranger.clone()], &none).is_err(),
        "a signer the log does not name grants nothing"
    );
    let managing = |account: [u8; 16]| {
        (account == other_id).then_some(Outside {
            level: Level::Manage,
            signing: other.signing(),
        })
    };
    assert!(
        check_log(LogKind::Block, block, pinned, &[root.clone(), stranger], &managing).is_ok(),
        "one who manages it from above grants it"
    );

    let organization = [9; 16];
    let root = Entry::root(&creator, creator_id, LogKind::Organization, organization);
    let pinned = root.hash();
    let make = |by: &RecoverySecret, signer, previous, account, identity, administrator| {
        Entry::sign(
            by,
            signer,
            organization,
            Some(previous),
            Record::Administrator(Administrator {
                account,
                identity,
                administrator,
            }),
        )
    };
    let promoted = make(&creator, creator_id, pinned, member_id, member.identity(), true);
    let demoted = make(&creator, creator_id, promoted.hash(), member_id, member.identity(), false);
    let checked = check_log(
        LogKind::Organization,
        organization,
        pinned,
        &[root.clone(), promoted.clone()],
        &none,
    )
    .unwrap();
    assert!(checked.administrators.contains_key(&member_id));
    let after = make(&member, member_id, demoted.hash(), other_id, other.identity(), true);
    assert!(
        check_log(
            LogKind::Organization,
            organization,
            pinned,
            &[root.clone(), promoted.clone(), demoted.clone(), after],
            &none,
        )
        .is_err(),
        "a demoted administrator makes no administrator"
    );
    let forged = make(&other, other_id, pinned, other_id, other.identity(), true);
    assert!(
        check_log(LogKind::Organization, organization, pinned, &[root, forged], &none).is_err(),
        "nor does anyone who never was one"
    );
}
