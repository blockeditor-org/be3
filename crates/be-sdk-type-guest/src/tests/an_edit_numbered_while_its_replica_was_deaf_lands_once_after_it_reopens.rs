use super::*;

/// The order numbered an edit while its replica heard nothing, and was sealed. The replica is
/// opened again (as after a restart of its module) and the host gives it the edit again, as the
/// order's holding it was never reported: the replica finds its own edit in the order and does
/// not make it twice.
#[test]
fn an_edit_numbered_while_its_replica_was_deaf_lands_once_after_it_reopens() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, add("start"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["start"]);

    sim.deaf.insert(1);
    let mine = add("mine");
    sim.operate(1, 1, mine.clone());
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(texts(&sim.content(0)), ["start", "mine"]);
    assert!(
        !sim.devices[1].status.clean,
        "the replica has not heard its edit numbered"
    );

    sim.deaf.clear();
    sim.reopen(1);
    sim.send_from_editor(
        1,
        1,
        FromEditor::Operate { gesture: None, operation: ChecklistContent::encode_operation(&mine) },
    );
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    for device in 0..2 {
        assert_eq!(
            texts(&sim.content(device)),
            ["start", "mine"],
            "device {device}"
        );
    }
    assert_eq!(texts(sim.mirror(1, 1)), ["start", "mine"]);
    assert_eq!(texts(&sim.head_content()), ["start", "mine"]);
    assert!(sim.devices[1].status.clean, "the edit settled");
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
