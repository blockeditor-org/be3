use super::*;

/// A ban (`Record::Ban`) checks only as a member with Edit or above signed it, lowers no one,
/// and stands until a grant to the banned account signed after it, by someone who may grant,
/// lifts it: the banned account revoking its own grant, which needs no Manage, lifts nothing.
#[test]
fn a_ban_checks_only_from_an_editor_and_only_a_grant_lifts_it() {
    let creator = RecoveryPhrase::generate().secret();
    let editor = RecoveryPhrase::generate().secret();
    let player = RecoveryPhrase::generate().secret();
    let (creator_id, editor_id, player_id) = ([1; 16], [2; 16], [3; 16]);
    let block = [7; 16];
    let none = |_: &Asked| None;
    let sign = |by: &RecoverySecret, signer, previous: &Entry, record| {
        Entry::sign(by, signer, block, Some(previous.hash()), false, record)
    };
    let grant = |grantee, identity, level| {
        Record::Grant(Grant {
            grantee,
            identity,
            level,
            authority: Authority::Log,
        })
    };
    let ban = |account| {
        Record::Ban(Ban {
            account,
            authority: Authority::Log,
        })
    };

    let root = Entry::root(&creator, creator_id, LogKind::Block, block);
    let pinned = root.hash();
    let editing = sign(
        &creator,
        creator_id,
        &root,
        grant(editor_id, editor.identity(), Level::Edit),
    );
    let viewing = sign(
        &creator,
        creator_id,
        &editing,
        grant(player_id, player.identity(), Level::View),
    );
    let check = |entries: &[Entry]| check_log(LogKind::Block, block, pinned, entries, &none);

    let banned = sign(&editor, editor_id, &viewing, ban(player_id));
    let mut log = vec![
        root.clone(),
        editing.clone(),
        viewing.clone(),
        banned.clone(),
    ];
    let checked = check(&log).expect("an editor bans");
    assert!(checked.banned.contains(&player_id));
    assert_eq!(
        checked.last_lowering, 0,
        "a ban lowers no one, so it outdates no epoch"
    );

    let by_viewer = sign(&player, player_id, &viewing, ban(editor_id));
    assert!(
        check(&[root.clone(), editing.clone(), viewing.clone(), by_viewer]).is_err(),
        "a viewer banned someone"
    );

    // The player leaves, which needs no Manage: the ban stands.
    let left = Entry::sign(
        &player,
        player_id,
        block,
        Some(banned.hash()),
        true,
        grant(player_id, player.identity(), Level::None),
    );
    log.push(left.clone());
    assert!(
        check(&log)
            .expect("anyone revokes their own grant")
            .banned
            .contains(&player_id),
        "the banned account lifted its own ban"
    );

    let regranted = sign(
        &creator,
        creator_id,
        &left,
        grant(player_id, player.identity(), Level::Join),
    );
    log.push(regranted);
    assert!(
        !check(&log)
            .expect("a manager grants again")
            .banned
            .contains(&player_id),
        "a grant after the ban lifts it"
    );
}
