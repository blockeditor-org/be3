use super::*;

/// A holder alone in its session streams edits faster than a seal's round trip, so the seal a
/// restart comes due on falls short of what the order numbered. It holds its own edits back and
/// seals again once what it submitted is numbered: that seal holds everything, the restart
/// follows it, and every edit, those held back included, lands.
#[test]
fn a_holder_streaming_alone_restarts_once_its_own_edits_are_numbered() {
    crate::replica::RESTART_LIMIT.with(|limit| limit.set(64));
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 0, "start\n");
    sim.advance(crate::QUIET_INTERVAL);

    // Every submission waits for the seal to be answered, as on a link slower than the typing.
    sim.stalled_submits = true;
    for round in 0..20 {
        sim.type_at(0, 1, 0, &format!("scratch {round} "));
        let typed = sim.typed(0, 1).find("start").expect("the text is there");
        sim.erase(0, 1, 0..typed);
    }
    sim.unstall();
    sim.stalled_submits = true;
    sim.type_at(0, 1, 0, "end ");
    sim.advance(crate::QUIET_INTERVAL);
    sim.type_at(0, 1, 0, "more ");
    sim.unstall();

    assert_eq!(sim.devices[0].status.restarts, 1, "the restart landed");
    assert_eq!(prose(&sim.content(0)), "more end start\n");
    assert_eq!(sim.typed(0, 1), "more end start\n");
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(
        prose(&sim.head_content()),
        "more end start\n",
        "and is sealed"
    );
}
