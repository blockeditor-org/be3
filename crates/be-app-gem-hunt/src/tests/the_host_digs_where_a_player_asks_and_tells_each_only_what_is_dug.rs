use super::*;

/// The host component is the rules: it tells each member the world as that member may see it
/// (never a gem not dug yet), digs where one asks when the cell may be dug, and tells every
/// member the world again once it changes.
#[test]
fn the_host_digs_where_a_player_asks_and_tells_each_only_what_is_dug() {
    let (ada, bo) = (Uuid::new_v4(), Uuid::new_v4());
    let mut test = BeuiTest::<HuntHost>::hosting(Uuid::new_v4());
    test.store().register(be_type_gem_hunt::registrations());
    test.hold(None, world(ada));
    test.run();

    let boards = |test: &mut BeuiTest<HuntHost>| -> Vec<(u64, Board)> {
        told(test)
            .into_iter()
            .filter_map(|command| match command {
                BlockCommand::TellMember(MemberSaid { member, payload }) => {
                    Some((member, decode(&payload)?))
                }
                _ => None,
            })
            .collect()
    };
    for (member, account) in [(1, ada), (2, bo)] {
        test.hear_member(MemberEvent::Joined(JoinedMember {
            member,
            account: account.into_bytes(),
            level: be_sdk_beui::AccessLevel::Join,
        }));
    }
    let joined = boards(&mut test);
    assert_eq!(
        joined.iter().map(|(member, _)| *member).collect::<Vec<_>>(),
        [1, 2],
        "each member is told the world as it joins"
    );
    let (_, first) = &joined[0];
    assert_eq!((first.found, first.gems, first.yours), (1, 6, 1));
    assert_eq!(
        first.dug.iter().map(|dug| dug.cell).collect::<Vec<_>>(),
        [0, 6],
        "only the dug cells, never where a gem still lies"
    );
    assert_eq!(
        first.dug[0].found,
        Found::Gem,
        "a gem's cell tells nothing of the gems around it"
    );
    assert_eq!(joined[1].1.yours, 0, "Bo found none");

    let ask = |test: &mut BeuiTest<HuntHost>, member, cell| {
        test.hear_member(MemberEvent::Said(MemberSaid {
            member,
            payload: encode(&Ask::Dig(cell)),
        }));
    };
    ask(&mut test, 2, 9);
    assert_eq!(
        test.content::<HuntContent>(None).root().dug.get(&9),
        Some(&bo),
        "the host writes Bo's dig to the block"
    );
    let after = boards(&mut test);
    assert_eq!(after.len(), 2, "both members hear of it");
    assert!(after.iter().all(|(_, board)| board.found == 2));
    assert_eq!(after[1].1.yours, 1, "Bo found the gem Bo dug");

    ask(&mut test, 1, 9);
    assert_eq!(
        test.content::<HuntContent>(None).root().dug.get(&9),
        Some(&bo),
        "a cell is dug once"
    );
    assert!(boards(&mut test).is_empty(), "and nothing changed to tell");

    test.hear_member(MemberEvent::Left(2));
    ask(&mut test, 2, 35);
    assert!(
        !test
            .content::<HuntContent>(None)
            .root()
            .dug
            .contains_key(&35),
        "one that left digs nothing"
    );
}
