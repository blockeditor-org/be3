use super::*;

/// The host seals what a replica owes on every hurried frame: an owner whose merge cannot read its
/// parts would have each seal rejected, so it owes none until the merge can run.
#[test]
fn an_owner_whose_merge_cannot_read_its_parts_owes_no_seal_until_it_can() {
    let mut sim = Sim::<ChecklistContent>::new(1, merging_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.operate(0, 1, add("unsealed"));
    sim.settle();
    assert!(
        sim.devices[0].status.owes_seal,
        "an unsealed edit is owed a seal"
    );
    let part = sim
        .vault
        .write(
            SEALING,
            Uuid::nil(),
            be_block_content::Part::from(&b"a closed segment"[..]),
        )
        .unwrap()
        .root
        .unwrap();
    PART.with(|held| held.set(Some(part)));
    sim.fail_reads = 1_000_000;
    let mut external = ChecklistContent::default();
    external.apply(&add("external"));
    sim.publish_elsewhere(&external);
    for _ in 0..10 {
        sim.advance(1_000);
    }
    assert!(
        !sim.devices[0].status.owes_seal,
        "a seal its failed merge would have rejected is owed"
    );

    sim.fail_reads = 0;
    for _ in 0..200 {
        sim.advance(1_000);
    }
    assert!(
        !sim.devices[0].status.owes_seal,
        "once the merge reads its parts, what was unsealed is sealed"
    );
}
