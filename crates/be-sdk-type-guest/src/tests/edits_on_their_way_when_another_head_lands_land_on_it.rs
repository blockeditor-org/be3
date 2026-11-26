use super::*;

/// Two edits are on their way, one submitted and one waiting behind it, when a head published
/// elsewhere lands, changing another line. The holder makes the head into what the order holds by
/// a merge that keeps the identities the edits name, and the order reloads it with them, so both
/// edits land on it, numbered after the reload or applied again on top of it.
#[test]
fn edits_on_their_way_when_another_head_lands_land_on_it() {
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 0, "one\ntwo\nthree");
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(prose(&sim.head_content()), "one\ntwo\nthree");

    sim.stalled_submits = true;
    sim.type_at(0, 1, 0, "A");
    sim.type_at(0, 1, 2, "B");
    assert_eq!(sim.typed(0, 1), "AoBne\ntwo\nthree");
    sim.publish_elsewhere(&prose_of("one\ntwo\nTHREE"));
    sim.unstall();
    sim.advance(crate::QUIET_INTERVAL);

    assert!(sim.conflicts.is_empty(), "{:?}", sim.conflicts);
    assert_eq!(prose(&sim.content(0)), "AoBne\ntwo\nTHREE");
    assert_eq!(prose(&sim.head_content()), "AoBne\ntwo\nTHREE");
    assert_eq!(sim.typed(0, 1), "AoBne\ntwo\nTHREE");
}
