use super::*;

#[test]
fn a_merge_by_parts_waits_for_the_parts_it_reads_and_never_resolves_without_them() {
    // A read that fails is tried again; the merge resolves once the part is read.
    let (mut sim, ours, theirs) = diverged();
    sim.fail_reads = 1;
    sim.open_at(0, Some(ours));
    assert_eq!(sim.part_reads, 2, "the part was read again after it failed");
    assert_eq!(READ.with(|read| read.borrow().clone()), b"a closed segment");
    assert_eq!(
        texts(&sim.head_content()),
        ["first, expanded", "second, revised"]
    );
    let head = sim.head.unwrap();
    assert_eq!(sim.commits[&head].parents, [theirs, ours]);
    assert!(sim.conflicts.is_empty());

    // A part that cannot be read leaves the merge unresolved, reported, and nothing published.
    let (mut sim, ours, theirs) = diverged();
    sim.fail_reads = 100;
    sim.open_at(0, Some(ours));
    assert_eq!(
        sim.head,
        Some(theirs),
        "the merge did not resolve without its part"
    );
    assert!(
        sim.errors
            .iter()
            .any(|(_, error)| error.contains("unreachable")),
        "the failure is reported: {:?}",
        sim.errors
    );
}
