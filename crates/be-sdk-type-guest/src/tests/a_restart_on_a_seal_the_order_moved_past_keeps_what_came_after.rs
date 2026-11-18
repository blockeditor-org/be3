use super::*;

/// An editor types and erases in rounds while its edits are on their way, and a seal comes due
/// past the size a restart starts at while the order is still numbering them. The restart goes on
/// that seal, which holds less than the order numbered: every replica carries what came after the
/// seal over it, so what was erased stays erased.
#[test]
fn a_restart_on_a_seal_the_order_moved_past_keeps_what_came_after() {
    crate::replica::RESTART_LIMIT.with(|limit| limit.set(64));
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 0, "start\n");
    sim.advance(crate::QUIET_INTERVAL);

    for round in 0..20 {
        sim.type_at(0, 1, 0, &format!("scratch {round} "));
        let typed = sim.typed(0, 1).find("start").expect("the text is there");
        sim.erase(0, 1, 0..typed);
    }
    sim.advance(crate::QUIET_INTERVAL);

    assert!(sim.devices[0].status.restarts >= 1, "the session restarted");
    assert_eq!(prose(&sim.content(0)), "start\n");
    assert_eq!(sim.typed(0, 1), "start\n");
}
