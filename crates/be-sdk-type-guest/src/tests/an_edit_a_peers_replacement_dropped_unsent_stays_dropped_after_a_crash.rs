use super::*;

/// A follower's edit is still on its way when another device replaces the content: the follower
/// drops it, keeps what it showed as a stash, and submits that stash to keep. It crashes before
/// its edit reaches the server. Opened again and given the edit again, it finds its stash in the
/// order counting the edit, so the edit is not sent onto the content that replaced it.
#[test]
fn an_edit_a_peers_replacement_dropped_unsent_stays_dropped_after_a_crash() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    assert!(sim.devices[0].status.holder);

    sim.stalled_submits = true;
    sim.operate(1, 1, Counter::add(1));
    sim.settle();
    let held = std::mem::take(&mut sim.held);
    assert_eq!(held.len(), 1, "the follower's edit was submitted");
    sim.stalled_submits = false;
    // The host last heard of the edit before it was dropped: it gives it again.
    let given = sim.devices[1].status.journaled_taken as usize;
    assert_eq!(given, 0);
    let mut replacement = CounterContent::default();
    replacement.apply(&Counter::add(100));
    sim.send_from_editor(0, 1, FromEditor::Replace(replacement.encode()));
    sim.settle();
    assert_eq!(
        count(&sim.content(1)),
        100,
        "the replacement dropped the edit"
    );
    assert!(
        sim.kept(|content| count(content) == 1),
        "the dropped edit was kept as a stash"
    );

    // The edit's first copy never reaches the server.
    sim.crash(1);
    drop(held);
    sim.open(1);
    sim.attach(1, 1, true);
    for by in [1].into_iter().skip(given) {
        sim.send_from_editor(
            1,
            1,
            FromEditor::Operate {
                gesture: None,
                operation: CounterContent::encode_operation(&Counter::add(by)),
            },
        );
    }
    sim.settle();
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    assert_eq!(
        count(&sim.head_content()),
        100,
        "the dropped edit was not sent again"
    );
    for device in 0..2 {
        assert_eq!(count(&sim.content(device)), 100, "device {device}");
    }
}
