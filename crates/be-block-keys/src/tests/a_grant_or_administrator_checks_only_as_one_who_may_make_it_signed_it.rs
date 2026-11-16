use super::*;

#[test]
fn a_grant_or_administrator_checks_only_as_one_who_may_make_it_signed_it() {
    let creator = RecoveryPhrase::generate().secret();
    let member = RecoveryPhrase::generate().secret();
    let other = RecoveryPhrase::generate().secret();
    let (creator_id, member_id, other_id) = ([1; 16], [2; 16], [3; 16]);
    let block = [7; 16];
    let none = |_: [u8; 16], _: &Authority| None;

    let root = Entry::root(&creator, creator_id, LogKind::Block, block);
    let pinned = root.hash();
    let granted = |by: &RecoverySecret, signer, previous, grantee, identity, level, authority| {
        Entry::sign(
            by,
            signer,
            block,
            Some(previous),
            Record::Grant(Grant {
                grantee,
                identity,
                level,
                authority,
            }),
        )
    };
    let grant = |by: &RecoverySecret, signer, previous, grantee, identity, level| {
        granted(
            by,
            signer,
            previous,
            grantee,
            identity,
            level,
            Authority::Log,
        )
    };
    let edit = grant(
        &creator,
        creator_id,
        pinned,
        member_id,
        member.identity(),
        Level::Edit,
    );
    let checked = check_log(
        LogKind::Block,
        block,
        pinned,
        &[root.clone(), edit.clone()],
        &none,
    )
    .unwrap();
    assert_eq!(
        checked.grants.get(&member_id),
        Some(&(Level::Edit, member.identity())),
        "the creator manages the block and grants it"
    );

    let onward = grant(
        &member,
        member_id,
        edit.hash(),
        other_id,
        other.identity(),
        Level::View,
    );
    assert!(
        check_log(
            LogKind::Block,
            block,
            pinned,
            &[root.clone(), edit.clone(), onward],
            &none
        )
        .is_err(),
        "a member with Edit grants no one"
    );
    let own = grant(
        &member,
        member_id,
        edit.hash(),
        member_id,
        member.identity(),
        Level::None,
    );
    assert!(
        check_log(
            LogKind::Block,
            block,
            pinned,
            &[root.clone(), edit.clone(), own],
            &none
        )
        .is_ok(),
        "anyone revokes their own grant"
    );
    let manager = grant(
        &creator,
        creator_id,
        pinned,
        member_id,
        member.identity(),
        Level::Manage,
    );
    let unmade = grant(
        &member,
        member_id,
        manager.hash(),
        creator_id,
        creator.identity(),
        Level::None,
    );
    assert!(
        check_log(
            LogKind::Block,
            block,
            pinned,
            &[root.clone(), manager, unmade],
            &none
        )
        .is_err(),
        "no grant lowers the creator, whose Manage is the root's"
    );
    let stranger = grant(
        &other,
        other_id,
        pinned,
        other_id,
        other.identity(),
        Level::Manage,
    );
    assert!(
        check_log(
            LogKind::Block,
            block,
            pinned,
            &[root.clone(), stranger],
            &none
        )
        .is_err(),
        "a signer the log does not name grants nothing"
    );
    let above = Authority::Inherited {
        block: [8; 16],
        head: pinned,
    };
    let from_above = granted(
        &other,
        other_id,
        pinned,
        other_id,
        other.identity(),
        Level::Manage,
        above,
    );
    let managing = |account: [u8; 16], authority: &Authority| {
        (account == other_id && *authority == above).then_some(other.signing())
    };
    assert!(
        check_log(
            LogKind::Block,
            block,
            pinned,
            &[root.clone(), from_above.clone()],
            &managing
        )
        .is_ok(),
        "one who manages it from above grants it, as of where the grant names"
    );
    assert!(
        check_log(
            LogKind::Block,
            block,
            pinned,
            &[root.clone(), from_above],
            &none
        )
        .is_err(),
        "and not once that log does not give it them as of there"
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
    let promoted = make(
        &creator,
        creator_id,
        pinned,
        member_id,
        member.identity(),
        true,
    );
    let demoted = make(
        &creator,
        creator_id,
        promoted.hash(),
        member_id,
        member.identity(),
        false,
    );
    let checked = check_log(
        LogKind::Organization,
        organization,
        pinned,
        &[root.clone(), promoted.clone()],
        &none,
    )
    .unwrap();
    assert!(checked.administrators.contains_key(&member_id));
    let after = make(
        &member,
        member_id,
        demoted.hash(),
        other_id,
        other.identity(),
        true,
    );
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
        check_log(
            LogKind::Organization,
            organization,
            pinned,
            &[root, forged],
            &none
        )
        .is_err(),
        "nor does anyone who never was one"
    );
}
