use super::*;

/// A checked log answers who held what as of any entry it reaches, and nothing for one it does
/// not, and which heads an entry lowering someone's access outdates: what a device checks a
/// key's epoch against (plans/signed_access.md, 3d).
#[test]
fn a_log_tells_each_level_as_of_each_of_its_entries() {
    let creator = RecoveryPhrase::generate().secret();
    let member = RecoveryPhrase::generate().secret();
    let (creator_id, member_id) = ([1; 16], [2; 16]);
    let block = [7; 16];
    let grant = |previous: EntryHash, level| {
        Entry::sign(
            &creator,
            creator_id,
            block,
            Some(previous),
            level == Level::None,
            Record::Grant(Grant {
                grantee: member_id,
                identity: member.identity(),
                level,
                authority: Authority::Log,
            }),
        )
    };
    let root = Entry::root(&creator, creator_id, LogKind::Block, block);
    let edit = grant(root.hash(), Level::Edit);
    let revoked = grant(edit.hash(), Level::None);
    let entries = [root.clone(), edit.clone(), revoked.clone()];
    let checked = check_log(LogKind::Block, block, root.hash(), &entries, &|_| None).unwrap();

    assert!(checked.holds(&root.hash()) && checked.holds(&revoked.hash()));
    assert_eq!(
        checked.level_as_of(creator_id, &root.hash()),
        Some(Level::Manage)
    );
    assert_eq!(checked.level_as_of(member_id, &root.hash()), None);
    assert_eq!(
        checked.level_as_of(member_id, &edit.hash()),
        Some(Level::Edit)
    );
    assert_eq!(
        checked.level_as_of(member_id, &revoked.hash()),
        Some(Level::None)
    );

    assert!(
        checked.outdates(&edit.hash()) && !checked.current_at(&edit.hash()),
        "an epoch named before a revocation is current"
    );
    assert!(checked.current_at(&revoked.hash()));

    let short = check_log(LogKind::Block, block, root.hash(), &entries[..2], &|_| None).unwrap();
    assert!(
        !short.holds(&revoked.hash()),
        "a log withheld before an entry reaches it"
    );
    assert_eq!(short.level_as_of(member_id, &revoked.hash()), None);
    assert!(
        short.current_at(&edit.hash()),
        "a log withheld before the revocation outdates nothing"
    );
}
