use super::*;

/// A replica whose origin the session's full base no longer names finds an entry of its own in
/// the journal: how far the order holds its edits is known exactly, so the edits it submitted
/// after, which never reached the server, are sent again rather than told they may have been.
#[test]
fn edits_after_an_own_entry_the_journal_holds_are_sent_again_though_the_base_names_no_longer_their_origin() {
    let others = crate::wire::BaseState::KEPT_ORIGINS + 4;
    let mut sim = Sim::<CounterContent>::new(others + 2, counter_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(0, 1, Counter::add(1));
    sim.settle();
    for device in 2..others + 2 {
        sim.open(device);
        sim.attach(device, 1, true);
        sim.operate(device, 1, Counter::add(1));
        sim.settle();
        sim.close(device);
    }
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    let before = 1 + others as i64;
    assert_eq!(count(&sim.head_content()), before);

    sim.operate(1, 1, Counter::add(10));
    sim.settle();
    sim.stalled_submits = true;
    sim.operate(1, 1, Counter::add(100));
    sim.operate(1, 1, Counter::add(1000));
    sim.settle();
    // What it submitted after never reached the server.
    sim.held.retain(|(device, _)| *device != 1);
    sim.stalled_submits = false;
    assert_eq!(count(&sim.content(0)), before + 10);

    sim.kept_submitted = 3;
    sim.reopen(1);
    for by in [100, 1000] {
        sim.send_from_editor(
            1,
            1,
            FromEditor::Operate(CounterContent::encode_operation(&Counter::add(by))),
        );
    }
    sim.settle();
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    assert_eq!(count(&sim.head_content()), before + 1110, "each edit landed once");
    assert_eq!(count(&sim.content(1)), before + 1110);
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
