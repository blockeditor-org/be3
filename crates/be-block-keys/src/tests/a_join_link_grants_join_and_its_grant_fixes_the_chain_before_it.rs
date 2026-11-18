use super::*;

#[test]
fn a_join_link_grants_join_and_its_grant_fixes_the_chain_before_it() {
    let owner = RecoveryPhrase::generate().secret();
    let player = RecoveryPhrase::generate().secret();
    let (owner_id, player_id) = ([1; 16], [3; 16]);
    let block = [7; 16];
    let none = |_: &Asked| None;
    let root = Entry::root(&owner, owner_id, LogKind::Block, block);
    let pinned = root.hash();
    let check = |entries: &[Entry]| check_log(LogKind::Block, block, pinned, entries, &none);
    let code = InviteCode::generate();
    let made = Entry::sign(
        &owner,
        owner_id,
        block,
        Some(pinned),
        false,
        Record::ShareLink(ShareLink {
            key: code.share_key(block),
            level: Level::Join,
            admits: Admits::Anyone,
            open: true,
            authority: Authority::Log,
        }),
    );
    let joined = Entry::opened(&code, block, made.hash(), player_id, &player, Level::Join);
    let checked = check(&[root.clone(), made.clone(), joined.clone()]).unwrap();
    assert_eq!(
        checked.grants.get(&player_id).map(|(level, _)| *level),
        Some(Level::Join),
        "a Join link grants Join"
    );
    let viewing = Entry::opened(&code, block, made.hash(), player_id, &player, Level::View);
    assert!(
        check(&[root.clone(), made.clone(), viewing]).is_err(),
        "a Join link grants no more"
    );

    // The opener's endorsement covers the entry its grant follows: moved after another entry,
    // even re-signed by the link's key, the grant no longer checks.
    let other = Entry::sign(
        &owner,
        owner_id,
        block,
        Some(made.hash()),
        false,
        Record::Opening(Opening {
            level: Level::View,
            authority: Authority::Log,
        }),
    );
    let Record::Opened(opened) = joined.record.clone() else {
        unreachable!()
    };
    let mut moved = joined.clone();
    moved.previous = Some(other.hash());
    moved.record = Record::Opened(opened);
    let resigned = Entry::invitation_signed(
        &code.share_signing(block),
        block,
        other.hash(),
        player_id,
        moved.record.clone(),
    );
    assert!(
        check(&[root, made, other, resigned]).is_err(),
        "the opener's keys fix where its grant stands in the log"
    );
}
