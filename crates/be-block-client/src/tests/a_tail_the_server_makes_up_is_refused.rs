use be_block_keys::{Entry, LogKind, Step};

/// What a device holds of a log is extended only by a tail that follows it: sent from where the
/// held log ends, whole, its first entry naming the held head. A server that says the tail starts
/// elsewhere, sends one that follows another head, reduces an entry in it, or sends a tail to a
/// device that holds nothing, is refused.
#[test]
fn a_tail_the_server_makes_up_is_refused() {
    let creator = be_block_keys::RecoveryPhrase::generate().secret();
    let (creator_id, block) = ([1; 16], [7; 16]);
    let root = Entry::root(&creator, creator_id, LogKind::Block, block);
    let next = |previous: be_block_keys::EntryHash, grantee: u8| {
        Entry::sign(
            &creator,
            creator_id,
            block,
            Some(previous),
            false,
            be_block_keys::Record::Grant(be_block_keys::Grant {
                grantee: [grantee; 16],
                identity: creator.identity(),
                level: be_block_keys::Level::View,
                authority: be_block_keys::Authority::Log,
            }),
        )
    };
    let first = next(root.hash(), 2);
    let held = vec![Step::from(root.clone()), Step::from(first.clone())];
    let tail = next(first.hash(), 3);

    let grown = crate::peer::extended(Some(&held), 2, vec![Step::from(tail.clone())]).unwrap();
    assert_eq!(
        grown.len(),
        3,
        "a tail that follows the held head extends it"
    );
    assert_eq!(
        crate::peer::extended(Some(&held), 0, vec![Step::from(root.clone())])
            .unwrap()
            .len(),
        1,
        "a log sent from its root replaces what was held"
    );

    assert!(
        crate::peer::extended(Some(&held), 1, vec![Step::from(tail.clone())]).is_none(),
        "a tail said to start before the held head ends"
    );
    assert!(
        crate::peer::extended(Some(&held), 3, vec![Step::from(tail.clone())]).is_none(),
        "a tail said to start past it"
    );
    let elsewhere = next(root.hash(), 4);
    assert!(
        crate::peer::extended(Some(&held), 2, vec![Step::from(elsewhere)]).is_none(),
        "a tail that follows another head"
    );
    assert!(
        crate::peer::extended(Some(&held), 2, vec![Step::Reduced(tail.reduced())]).is_none(),
        "a tail with an entry reduced"
    );
    assert!(
        crate::peer::extended(None, 2, vec![Step::from(tail)]).is_none(),
        "a tail to a device that holds nothing"
    );
}
