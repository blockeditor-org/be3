use super::*;

#[test]
fn a_merge_that_cannot_read_its_parts_is_reported_once_and_tried_again_on_a_timer() {
    let (mut sim, ours, theirs) = diverged();
    sim.fail_reads = 6;
    sim.open_at(0, Some(ours));
    assert_eq!(
        sim.head,
        Some(theirs),
        "the merge did not resolve without its part"
    );
    assert_eq!(sim.errors.len(), 1, "the failure is reported once");
    sim.advance(5_000);
    assert_eq!(sim.head, Some(theirs), "the second try failed too");
    assert_eq!(sim.errors.len(), 1, "and is not reported again");
    // The parts can be read: the next try, after the doubled wait, resolves the merge.
    sim.advance(10_000);
    let head = sim.head.unwrap();
    assert_eq!(sim.commits[&head].parents, [theirs, ours]);
    assert_eq!(
        texts(&sim.head_content()),
        ["first, expanded", "second, revised"]
    );
}
