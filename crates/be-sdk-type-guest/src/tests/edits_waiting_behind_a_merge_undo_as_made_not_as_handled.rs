use super::*;

/// Edits a second apart that wait as jobs (here behind a merge waiting for its part) are
/// recorded for undo as made when the host relayed them, not as the replica handles them, all
/// at once: they stay two steps rather than absorbing into one burst.
#[test]
fn edits_waiting_behind_a_merge_undo_as_made_not_as_handled() {
    let mut sim = live();
    let (item, created) = Checklist::add("Quick");
    sim.operate(0, 1, created);
    sim.advance(1_000);
    let mut external = ChecklistContent::default();
    external.apply(&add("external"));
    sim.read_stalled = true;
    sim.publish_elsewhere(&external);

    sim.operate(0, 1, Checklist::set_text(item, "Quickly"));
    sim.advance(1_000);
    sim.operate(0, 1, Checklist::set_text(item, "Quickly done"));
    sim.unstall();

    let quick = |sim: &mut Sim<ChecklistContent>| {
        texts(&sim.content(0))
            .into_iter()
            .filter(|text| text.starts_with("Quick"))
            .collect::<Vec<_>>()
    };
    assert_eq!(quick(&mut sim), ["Quickly done"]);
    assert!(sim.change(0, Change::Undo));
    assert_eq!(
        quick(&mut sim),
        ["Quickly"],
        "the edit made a second after the one before it is a step of its own"
    );
}
