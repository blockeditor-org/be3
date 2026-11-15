use super::*;

/// The order numbered an edit while its replica heard nothing, and the holder sealed it and
/// trimmed the journal over several quiet intervals, so no entry holds it any more. The replica
/// is opened again and given the edit again: the journal's base says how far the order holds
/// its edits, so it does not make it twice.
#[test]
fn an_edit_numbered_while_its_replica_was_deaf_lands_once_after_another_seals_it() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(0, 1, Counter::add(1));
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);

    sim.deaf.insert(1);
    sim.operate(1, 1, Counter::add(10));
    sim.settle();
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    assert_eq!(count(&sim.head_content()), 11, "the holder sealed the edit");
    assert!(
        sim.journal
            .as_ref()
            .is_some_and(|journal| journal.entries.is_empty()),
        "no entry holds the edit any more"
    );

    sim.deaf.clear();
    sim.reopen(1);
    sim.send_from_editor(
        1,
        1,
        FromEditor::Operate(CounterContent::encode_operation(&Counter::add(10))),
    );
    sim.settle();
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    for device in 0..2 {
        assert_eq!(count(&sim.content(device)), 11, "device {device}");
    }
    assert_eq!(count(&sim.head_content()), 11);
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
