use super::*;

/// An edit too large to submit inline is journaled by its part. A holder that crashed before
/// its commit landed leaves that entry, and the next replica to open reads the part before it
/// replays the journal.
#[test]
fn a_new_holder_replays_a_journaled_edit_by_its_part() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 6, "typed\n");
    // Its seals fail, as offline with its key gone, so it stops with the paste unsealed.
    sim.fail_publishes = usize::MAX;
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(0, 1, 12, &pasted);
    sim.settle();
    let journal = sim.journal.as_ref().expect("the owner journaled its edits");
    assert!(
        journal
            .entries
            .iter()
            .all(|entry| entry.payload.len() < 1024 * 1024),
        "the journal holds the paste by its part"
    );
    assert!(
        sim.failed_publishes > 0,
        "the holder was committing what the order holds"
    );
    sim.crash(0);
    sim.fail_publishes = 0;
    sim.errors.clear();

    let reads = sim.part_reads;
    sim.open(1);
    assert_eq!(sim.part_reads, reads + 1, "the replica read the part");
    let expected = format!("start\ntyped\n{pasted}");
    assert!(prose(&sim.content(1)) == expected);
    assert!(sim.devices[1].status.holder);
    sim.advance(0);
    assert!(prose(&sim.head_content()) == expected);
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
