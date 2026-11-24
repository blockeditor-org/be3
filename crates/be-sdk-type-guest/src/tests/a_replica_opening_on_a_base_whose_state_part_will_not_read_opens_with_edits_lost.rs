use super::*;

/// The holder's base goes by a part. A replica opening on it reads the part again while reads
/// fail, and opens as the participants hold the block once one lands. When the part never reads,
/// it gives the part up: the journal's edits were made on that state, so it opens on the
/// commit with them lost, and says so. So it does for an operation's part that never reads: it
/// would otherwise replay without an edit every participant has.
#[test]
fn a_replica_opening_on_a_base_whose_state_part_will_not_read_opens_with_edits_lost() {
    crate::replica::LARGEST_INLINE.with(|largest| largest.set(64));
    let mut sim = Sim::<ProseContent>::new(4, prose_module);
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
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.base_parts.len(), 1, "the base goes by a part");
    sim.type_at(1, 1, 0, "c ");
    sim.settle();

    // Reads fail twice, then land: the replica opens on the journal as the others hold it.
    sim.fail_reads = 2;
    sim.open(2);
    sim.attach(2, 1, true);
    for _ in 0..10 {
        sim.advance(1_000);
    }
    assert_eq!(prose(&sim.content(2)), "c b start\n");
    assert!(!sim.devices[2].undo.lost_edits);

    // Reads never land: it gives the part up and opens on the commit, edits lost.
    sim.fail_reads = usize::MAX;
    sim.open(3);
    sim.attach(3, 1, true);
    for _ in 0..20 {
        sim.advance(60_000);
    }
    assert_eq!(prose(&sim.content(3)), "b start\n", "it opens on the commit");
    assert!(sim.devices[3].undo.lost_edits, "and says edits were lost");

    // An operation the journal holds goes by its part, which never reads: the replica opens on
    // the commit with the journal's edits lost, rather than replay without it.
    let mut sim = Sim::<ProseContent>::new(3, prose_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.type_at(0, 1, 0, "start\n");
    sim.advance(crate::QUIET_INTERVAL);
    let large = "x".repeat(100);
    sim.type_at(1, 1, 0, &large);
    sim.settle();
    assert_eq!(prose(&sim.content(0)), format!("{large}start\n"));
    sim.fail_reads = usize::MAX;
    sim.open(2);
    sim.attach(2, 1, true);
    for _ in 0..20 {
        sim.advance(60_000);
    }
    assert_eq!(prose(&sim.content(2)), "start\n", "it opens on the commit");
    assert!(sim.devices[2].undo.lost_edits, "and says edits were lost");
}
