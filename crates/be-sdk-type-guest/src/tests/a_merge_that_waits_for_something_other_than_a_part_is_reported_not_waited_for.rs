use super::*;

#[test]
fn a_merge_that_waits_for_something_other_than_a_part_is_reported_not_waited_for() {
    let mut sim = Sim::<ChecklistContent>::new(1, waiting_module);
    let mut base = ChecklistContent::default();
    base.apply(&add("first"));
    let shared = sim.publish_elsewhere(&base);
    let theirs = sim.publish_elsewhere(&{
        let mut edited = base.clone();
        edited.apply(&add("theirs"));
        edited
    });
    let mut edited = base.clone();
    edited.apply(&add("ours"));
    let ours = sim.write(edited.encode(), vec![shared]);
    sim.open_at(0, Some(ours));
    assert_eq!(sim.head, Some(theirs), "nothing was published");
    assert!(
        sim.errors
            .iter()
            .any(|(_, error)| error.contains("other than a part read")),
        "it is reported: {:?}",
        sim.errors
    );
    // The replica is not stuck behind it: it closes.
    sim.close(0);
}
