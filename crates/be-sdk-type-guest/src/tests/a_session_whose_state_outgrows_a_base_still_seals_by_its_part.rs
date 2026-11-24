use super::*;

/// A session whose state passes what a journal's base carries inline (the server's
/// `MAX_BASE_STATE_BYTES`), with a peer in it so no restart comes due, still seals: the holder
/// puts the state as a part, the base names it, and the server keeps it while it does. A replica
/// opening on that base reads the part and holds what the participants hold.
#[test]
fn a_session_whose_state_outgrows_a_base_still_seals_by_its_part() {
    crate::replica::LARGEST_INLINE.with(|largest| largest.set(64));
    let mut sim = Sim::<ProseContent>::new(3, prose_module);
    sim.largest_base_state = 128;
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.type_at(0, 1, 0, "start\n");
    for round in 0..40 {
        sim.type_at(0, 1, 0, &format!("scratch {round} "));
        let typed = sim.typed(0, 1).find("start").expect("the text is there");
        sim.erase(0, 1, 0..typed);
    }
    sim.type_at(1, 1, 0, "b ");
    sim.settle();
    let before = sim.head;
    sim.advance(crate::QUIET_INTERVAL);

    assert_eq!(sim.failed_publishes, 0, "no seal failed");
    assert_ne!(sim.head, before, "the seal moved the head");
    assert_eq!(sim.devices[0].status.restarts, 0, "no restart came due");
    assert_eq!(prose(&sim.head_content()), "b start\n");
    assert_eq!(
        sim.base_parts.len(),
        1,
        "the base names the part its state goes by"
    );
    let part = be_block_store::PartRef::from(sim.base_parts[0].clone());
    assert!(
        part.length() > 128,
        "the state is past what a base carries: {}",
        part.length()
    );
    let state = &sim.journal.as_ref().expect("a journal").base.state;
    assert!(
        state.len() <= 128,
        "the base carries the part, not the state"
    );

    // What comes after the seal was made on that state: a replica opening on the base reads
    // the part and replays it alike.
    sim.type_at(1, 1, 0, "c ");
    sim.settle();
    sim.open(2);
    sim.attach(2, 1, true);
    sim.settle();
    assert_eq!(prose(&sim.content(2)), "c b start\n");
    sim.type_at(2, 1, 0, "d ");
    sim.settle();
    assert_eq!(prose(&sim.content(0)), "d c b start\n");
    assert_eq!(prose(&sim.content(1)), "d c b start\n");
}
