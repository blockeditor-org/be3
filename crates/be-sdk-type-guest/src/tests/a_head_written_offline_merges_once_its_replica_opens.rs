use super::*;

fn diverged(theirs: &str, ours: &str) -> (Sim<ChecklistContent>, CommitId, CommitId) {
    let mut sim = Sim::<ChecklistContent>::new(1, checklist_module);
    let mut base = ChecklistContent::default();
    let (first, created) = Checklist::add("first");
    base.apply(&created);
    let (second, created) = Checklist::add("second");
    base.apply(&created);
    let shared = sim.publish_elsewhere(&base);
    let edit = |content: &ChecklistContent, change: &str| {
        let mut edited = content.clone();
        let (item, text) = change.split_once('=').unwrap();
        let item = if item == "first" { first } else { second };
        edited.apply(&Checklist::set_text(item, text));
        edited
    };
    let theirs = sim.publish_elsewhere(&edit(&base, theirs));
    let ours = sim.write(edit(&base, ours).encode(), vec![shared]);
    (sim, ours, theirs)
}

#[test]
fn a_head_written_offline_merges_once_its_replica_opens() {
    let (mut sim, ours, theirs) = diverged("second=second, revised", "first=first, expanded");
    sim.open_at(0, Some(ours));
    assert!(
        sim.conflicts.is_empty(),
        "edits to separate items conflicted"
    );
    assert_eq!(
        texts(&sim.head_content()),
        ["first, expanded", "second, revised"]
    );
    let head = sim.head.unwrap();
    assert_eq!(sim.commits[&head].parents, [theirs, ours]);
    assert_eq!(texts(&sim.content(0)), texts(&sim.head_content()));

    let (mut sim, ours, theirs) = diverged("second=theirs", "second=ours");
    sim.open_at(0, Some(ours));
    let [(_, kept, conflicted_with, count)] = sim.conflicts.as_slice() else {
        panic!("one conflict is reported, not {:?}", sim.conflicts);
    };
    assert_eq!((*kept, *conflicted_with), (ours, theirs));
    assert!(*count > 0);
    let head = sim.head.unwrap();
    assert_eq!(sim.commits[&head].parents, [theirs, ours]);
    assert_eq!(texts(&sim.content(0)), texts(&sim.head_content()));
}
