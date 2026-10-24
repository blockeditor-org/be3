use super::*;

#[test]
fn a_conflicting_merge_keeps_the_unsealed_side_as_a_commit() {
    let mut sim = Sim::<ChecklistContent>::new(1, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    let (item, created) = Checklist::add("base");
    sim.operate(0, 1, created);
    sim.advance(crate::SEAL_INTERVAL);
    let base = sim.head.expect("the base was sealed");
    sim.operate(0, 1, Checklist::set_text(item, "ours"));
    sim.settle();
    let mut theirs = sim.head_content();
    theirs.apply(&Checklist::set_text(item, "theirs"));
    let external = sim.publish_elsewhere(&theirs);
    let [(_, ours, conflicted_with, count)] = sim.conflicts.as_slice() else {
        panic!("one conflict is reported, not {:?}", sim.conflicts);
    };
    assert_eq!(*conflicted_with, external);
    assert!(*count > 0);
    assert_eq!(sim.commits[ours].parents, [base]);
    let kept = ChecklistContent::decode(&sim.commits[ours].bytes).unwrap();
    assert_eq!(texts(&kept), ["ours"]);
    let head = sim.head.unwrap();
    assert_eq!(sim.commits[&head].parents, [external, *ours]);
    assert_eq!(texts(&sim.content(0)), texts(&sim.head_content()));
    assert_eq!(texts(sim.mirror(0, 1)), texts(&sim.head_content()));
}
