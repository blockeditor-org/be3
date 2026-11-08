use super::*;

fn base_state(sim: &Sim<ProseContent>) -> usize {
    sim.journal
        .as_ref()
        .map_or(0, |journal| journal.base.state.len())
}

#[test]
fn a_restart_waits_for_the_owner_to_be_alone_below_the_ceiling() {
    crate::replica::RESTART_LIMIT.with(|limit| limit.set(usize::MAX));
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    for round in 0..20 {
        sim.type_at(0, 1, 0, &format!("scratch {round} "));
        let typed = sim.typed(0, 1).find("start").unwrap();
        sim.erase(0, 1, 0..typed);
    }
    sim.type_at(0, 1, 0, "kept ");
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.publishes, 1);
    let state = base_state(&sim);
    assert!(state > 0, "the commit's base keeps the session state");

    crate::replica::RESTART_LIMIT.with(|limit| limit.set(state / 2));
    sim.type_at(0, 1, 0, "more ");
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.publishes, 2);
    assert!(
        base_state(&sim) >= state,
        "below the ceiling, an owner with others in the session does not restart"
    );

    sim.close(1);
    sim.type_at(0, 1, 0, "alone ");
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.publishes, 3);
    assert!(
        base_state(&sim) < state / 2,
        "alone, the owner restarts at its next commit"
    );
    assert_eq!(prose(&sim.content(0)), "alone more kept start\n");
}
