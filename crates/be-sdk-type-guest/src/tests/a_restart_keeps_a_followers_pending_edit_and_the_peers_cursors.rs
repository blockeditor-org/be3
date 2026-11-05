use super::*;

const CURSOR: Uuid = Uuid::from_u128(0xc0);

#[test]
fn a_restart_keeps_a_followers_pending_edit_and_the_peers_cursors() {
    crate::replica::RESTART_LIMIT.with(|limit| limit.set(8));
    let mut sim = Sim::<ProseContent>::new(2, prose_with_undo_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.send_from_editor(
        0,
        1,
        FromEditor::ShowPresence {
            kind: CURSOR,
            value: Some(vec![3]),
        },
    );
    for round in 0..20 {
        sim.type_at(0, 1, 0, &format!("scratch {round} "));
        let typed = sim.typed(0, 1).find("start").unwrap();
        sim.erase(0, 1, 0..typed);
    }
    sim.type_at(1, 1, 0, "early ");
    sim.settle();
    assert!(sim.devices[1].undo.can_undo);

    sim.deaf.insert(0);
    sim.type_at(1, 1, 0, "pending ");
    sim.settle();
    sim.deaf.clear();
    sim.advance(crate::QUIET_INTERVAL);
    sim.settle();

    let restarted = sim.head;
    assert!(restarted.is_some());
    assert!(
        sim.journal
            .as_ref()
            .is_some_and(|journal| journal.base.commit == restarted && journal.base.state.len() < 8),
        "the owner reloaded from the commit and dropped its tombstones"
    );
    for device in 0..2 {
        assert_eq!(
            prose(&sim.content(device)),
            "pending early start\n",
            "the follower's pending edit moved onto the reloaded content"
        );
        assert_eq!(sim.typed(device, 1), "pending early start\n");
    }
    assert_eq!(
        sim.editors[&(1, 1)]
            .presence()
            .iter()
            .map(|peer| peer.value.clone())
            .collect::<Vec<_>>(),
        [vec![3]],
        "a cursor in visible offsets carries over"
    );

    let undo = sim.devices[1].undo.clone();
    assert!(!undo.can_undo, "no step from before the restart applies");
    assert!(undo.restarted_at.is_some(), "undo stops at the restart");
    assert!(!sim.change(1, Change::Undo));
    assert_eq!(prose(&sim.content(1)), "pending early start\n");

    sim.type_at(1, 1, 0, "after ");
    sim.settle();
    assert!(sim.devices[1].undo.can_undo);
    assert!(sim.change(1, Change::Undo));
    sim.settle();
    assert_eq!(prose(&sim.content(1)), "pending early start\n");
    assert!(!sim.devices[1].undo.can_undo);
    assert_eq!(sim.devices[1].undo.restarted_at, undo.restarted_at);
}
