use super::*;

/// An edit is on its way when the session restarts, and a second is typed behind it, waiting
/// for the first to come back. Carried over, each goes again holding the local edits it held
/// (`Submitted::through`): the first its own alone, though the second was typed when it was
/// carried. The carried first is kept, and the replica is opened again before it is numbered:
/// given again, the first goes under the kept submission's id and the second anew, so each
/// lands, the first once though its first copy reaches the server late.
#[test]
fn two_edits_carried_over_a_restart_each_go_again_with_the_edits_they_hold() {
    crate::replica::RESTART_LIMIT.with(|limit| limit.set(8));
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    let mut base = ChecklistContent::default();
    let (first, created) = Checklist::add("first");
    base.apply(&created);
    let (second, created) = Checklist::add("second");
    base.apply(&created);
    sim.publish_elsewhere(&base);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    // What a restart drops: items made and removed again.
    for round in 0..20 {
        let (item, created) = Checklist::add(&format!("scratch {round}"));
        sim.operate(0, 1, created);
        sim.operate(0, 1, Checklist::remove(item));
    }
    sim.settle();

    // As the editor sends them, which the host keeps to give again. Each names its item, which
    // keeps its id over the restart, so carried over it goes as it was typed.
    let edit = |item, text: &str| FromEditor::Operate {
        gesture: None,
        operation: ChecklistContent::encode_operation(&Checklist::set_text(item, text)),
    };
    let (on_its_way, behind) = (edit(first, "first, revised"), edit(second, "second, revised"));
    // Its submissions are held on their way: the first, and the one carried over the restart
    // that comes while it is on its way. The second waits behind the first throughout.
    sim.stalled_from = Some(1);
    sim.send_from_editor(1, 1, on_its_way.clone());
    sim.settle();
    sim.send_from_editor(1, 1, behind.clone());
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    sim.settle();
    let held: Vec<be_sdk_type_api::Submission> = std::mem::take(&mut sim.held)
        .into_iter()
        .filter_map(|(device, call)| match call {
            Call::Submit { submission, .. } if device == 1 => Some(submission),
            _ => None,
        })
        .collect();
    sim.stalled_from = None;
    let [as_typed, carried] =
        <[_; 2]>::try_from(held).expect("submitted as typed and carried over");
    let restarts = sim.devices[1].status.restarts;
    assert!(restarts > 0, "the session restarted");
    let held = Submitted::decode(&carried.payload).expect("a submission decodes");
    assert_eq!(held.through, 1, "the carried first holds its own edit alone");

    sim.kept_opening = Some((restarts, 0));
    sim.kept_submitted = held.through;
    sim.kept_submissions = vec![be_sdk_type_api::KeptSubmission {
        counter: carried.counter,
        through: held.through,
        payload: carried.payload.clone(),
    }];
    sim.given_again = vec![(1, on_its_way), (1, behind)];
    // The submissions taken on their way are never answered: the replica they went from is
    // opened again.
    sim.errors.clear();
    sim.reopen(1);
    sim.settle();
    // Both copies the crashed process sent reach the server after what is given again: the
    // first, as typed, is passed over as made before the restart.
    sim.number(1, as_typed);
    sim.number(1, carried);
    sim.settle();
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    let revised = ["first, revised", "second, revised"];
    assert_eq!(texts(&sim.head_content()), revised, "each edit landed");
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), revised, "device {device}");
    }
    assert!(sim.devices[1].status.clean, "the edits settled");
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
