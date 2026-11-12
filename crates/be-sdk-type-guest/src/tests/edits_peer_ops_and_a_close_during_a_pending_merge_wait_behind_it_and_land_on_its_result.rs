use super::*;

#[test]
fn edits_peer_ops_and_a_close_during_a_pending_merge_wait_behind_it_and_land_on_its_result() {
    let mut sim = live();
    let mut external = ChecklistContent::default();
    external.apply(&add("external"));
    sim.read_stalled = true;
    sim.publish_elsewhere(&external);
    assert_eq!(sim.publishes, 0, "the merge waits for its part");

    // The owner's editor and a follower type while it waits: both go on top of the merged value.
    sim.operate(0, 1, add("during"));
    sim.operate(1, 1, add("peer"));
    sim.unstall();
    sim.advance(1_000);
    let mut merged = texts(&sim.content(0));
    merged.sort();
    assert_eq!(merged, ["during", "external", "peer", "unsealed"]);
    assert_eq!(
        texts(&sim.content(1)).len(),
        4,
        "the follower holds them all too"
    );
    assert_eq!(READ.with(|read| read.borrow().clone()), b"a closed segment");

    // A close while a merge waits ends it: the replica seals what it had and goes, and the merge
    // that was waiting publishes nothing of its own.
    let mut sim = live();
    sim.read_stalled = true;
    sim.publish_elsewhere(&external);
    let before = sim.publishes;
    sim.close(0);
    sim.unstall();
    assert!(
        sim.publishes <= before + 1,
        "only the seal of the close was published"
    );
    assert!(
        texts(&sim.head_content()).contains(&"unsealed".to_owned()),
        "the edit the replica had is kept"
    );
    assert!(
        !texts(&sim.head_content()).contains(&"external".to_owned()) || sim.publishes > before,
        "nothing was lost"
    );
}
