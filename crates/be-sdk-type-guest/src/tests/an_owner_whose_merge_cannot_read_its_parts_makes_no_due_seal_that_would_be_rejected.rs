use super::*;

#[test]
fn an_owner_whose_merge_cannot_read_its_parts_makes_no_due_seal_that_would_be_rejected() {
    let mut sim = Sim::<ChecklistContent>::new(1, merging_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.operate(0, 1, add("unsealed"));
    sim.settle();
    let part = sim
        .vault
        .write(
            SEALING,
            Uuid::nil(),
            be_block::Part::from(&b"a closed segment"[..]),
        )
        .unwrap()
        .root
        .unwrap();
    PART.with(|held| held.set(Some(part)));
    sim.fail_reads = 1_000_000;
    let mut external = ChecklistContent::default();
    external.apply(&add("external"));
    sim.publish_elsewhere(&external);
    let rejected = sim.rejections;
    for _ in 0..80 {
        sim.advance(1_000);
    }
    assert!(
        sim.rejections <= rejected + 1,
        "the seal due on an old base is not tried every tick: {} rejected",
        sim.rejections - rejected
    );
}
