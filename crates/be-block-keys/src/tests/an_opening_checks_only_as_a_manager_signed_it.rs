use super::*;

#[test]
fn an_opening_checks_only_as_a_manager_signed_it() {
    let creator = RecoveryPhrase::generate().secret();
    let member = RecoveryPhrase::generate().secret();
    let (creator_id, member_id) = ([1; 16], [2; 16]);
    let block = [7; 16];
    let none = |_: &Asked| None;
    let check =
        |entries: &[Entry]| check_log(LogKind::Block, block, entries[0].hash(), entries, &none);
    let opening = |by: &RecoverySecret, signer, previous, level, authority| {
        Entry::sign(
            by,
            signer,
            block,
            Some(previous),
            level == Level::None,
            Record::Opening(Opening { level, authority }),
        )
    };

    let root = Entry::root(&creator, creator_id, LogKind::Block, block);
    assert_eq!(check(&[root.clone()]).unwrap().open_inside, Level::None);
    let opened = opening(
        &creator,
        creator_id,
        root.hash(),
        Level::View,
        Authority::Log,
    );
    let checked = check(&[root.clone(), opened.clone()]).unwrap();
    assert_eq!(
        checked.open_inside,
        Level::View,
        "the creator opens the block"
    );

    let closed = opening(
        &creator,
        creator_id,
        opened.hash(),
        Level::None,
        Authority::Log,
    );
    assert_eq!(
        check(&[root.clone(), opened.clone(), closed])
            .unwrap()
            .open_inside,
        Level::None,
        "the last opening holds"
    );

    let edit = Entry::sign(
        &creator,
        creator_id,
        block,
        Some(root.hash()),
        false,
        Record::Grant(Grant {
            grantee: member_id,
            identity: member.identity(),
            level: Level::Edit,
            authority: Authority::Log,
        }),
    );
    let by_editor = opening(&member, member_id, edit.hash(), Level::View, Authority::Log);
    assert!(
        check(&[root.clone(), edit, by_editor]).is_err(),
        "one who does not manage the block opens nothing"
    );
    for level in [Level::KnowExists, Level::Edit, Level::Manage] {
        let other = opening(&creator, creator_id, root.hash(), level, Authority::Log);
        assert!(
            check(&[root.clone(), other]).is_err(),
            "a block opens to anyone inside only to view, or closes"
        );
    }
    let inherited = opening(
        &member,
        member_id,
        root.hash(),
        Level::View,
        Authority::Inherited(vec![Link {
            block: [8; 16],
            head: [0; 32],
        }]),
    );
    let above = |_: &Asked| Some(member.identity().signing);
    let checked = check_log(
        LogKind::Block,
        block,
        root.hash(),
        &[root.clone(), inherited],
        &above,
    )
    .unwrap();
    assert_eq!(
        checked.open_inside,
        Level::View,
        "a manager from above opens the block, as the logs above answer"
    );
}
