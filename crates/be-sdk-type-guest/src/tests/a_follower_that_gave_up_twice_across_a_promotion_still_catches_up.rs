use super::*;

/// A follower that gave up on a part, became the owner and followed again, and gives up on
/// another part, asks to be caught up again rather than ignoring the owner for good.
#[test]
fn a_follower_that_gave_up_twice_across_a_promotion_still_catches_up() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.lease = Lease::lasting(0);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.stalled = true;
    sim.fail_reads = usize::MAX;
    let first = "a line of pasted text\n".repeat(60_000);
    sim.type_at(0, 1, 6, &first);
    sim.settle();
    sim.advance(30_000);
    sim.fail_reads = 0;
    sim.unstall();
    sim.advance(60_000);
    sim.claim(1);
    sim.settle();
    sim.claim(0);
    sim.settle();
    assert!(sim.devices[0].status.owner, "the first owner took it back");

    sim.stalled = true;
    sim.fail_reads = usize::MAX;
    let second = "another pasted line\n".repeat(60_000);
    sim.type_at(0, 1, 0, &second);
    sim.settle();
    sim.advance(30_000);
    sim.fail_reads = 0;
    sim.unstall();
    sim.advance(120_000);
    let text = sim.typed(0, 1);
    assert_eq!(text.len(), 6 + first.len() + second.len());
    assert!(sim.typed(1, 1) == text, "the follower caught up");
    assert!(prose(&sim.content(1)) == text);
}
