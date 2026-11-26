use super::*;

/// An edit is on its way when the session restarts, and a second is typed behind it. Both are
/// carried over and kept, and nothing crashes: when the first lands, the second goes as what it
/// was carried to, which its editor already shows, so the editor is not sent the content afresh.
#[test]
fn an_edit_carried_over_a_restart_live_sends_its_editor_no_snapshot() {
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

    // The first is held on its way, with the one carried over the restart that comes while it
    // is; the second waits behind it throughout.
    sim.stalled_from = Some(1);
    sim.operate(1, 1, Checklist::set_text(first, "first, revised"));
    sim.settle();
    sim.operate(1, 1, Checklist::set_text(second, "second, revised"));
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
    assert!(sim.devices[1].status.restarts > 0, "the session restarted");
    sim.take_received(1, 1);

    sim.number(1, as_typed);
    sim.number(1, carried);
    sim.settle();
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    assert!(
        !sim.take_received(1, 1)
            .iter()
            .any(|message| matches!(message, ToEditor::Snapshot(_))),
        "the second went as its editor shows it"
    );
    let revised = ["first, revised", "second, revised"];
    assert_eq!(texts(&sim.head_content()), revised, "each edit landed");
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), revised, "device {device}");
    }
    assert_eq!(texts(sim.mirror(1, 1)), revised, "the editor shows both");
    assert!(sim.devices[1].status.clean, "the edits settled");
    // The calls taken on their way are dropped unanswered, which is all that is told: the
    // server numbers what they sent instead.
    assert!(
        sim.errors
            .iter()
            .all(|(_, error)| error.contains("dropped the operation without answering it")),
        "{:?}",
        sim.errors
    );
}
