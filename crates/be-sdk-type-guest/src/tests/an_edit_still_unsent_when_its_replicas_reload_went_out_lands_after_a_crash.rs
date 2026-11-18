use super::*;

/// The holder's edit waits behind one on its way when the head moves and the holder submits a
/// reload of it. It crashes before the edit goes out, and the server numbers what it had
/// submitted. Opened again and given its edits again, it finds the reload of its own origin in
/// the order, which does not count the edit that was still unsent, so the edit is sent and lands.
#[test]
fn an_edit_still_unsent_when_its_replicas_reload_went_out_lands_after_a_crash() {
    let mut sim = Sim::<CounterContent>::new(1, counter_module);
    sim.open(0);
    sim.attach(0, 1, true);

    sim.stalled_submits = true;
    sim.operate(0, 1, Counter::add(1));
    sim.operate(0, 1, Counter::add(10));
    sim.settle();
    let mut external = CounterContent::default();
    external.apply(&Counter::add(100));
    sim.publish_elsewhere(&external);
    let given = sim.devices[0].status.journaled_taken as usize;

    sim.crash(0);
    let held = std::mem::take(&mut sim.held);
    assert_eq!(
        held.len(),
        2,
        "the first edit and the reload were submitted"
    );
    for (device, call) in held {
        let Call::Submit { submission, .. } = call else {
            panic!("only submissions are held");
        };
        sim.number(device, submission);
    }
    sim.stalled_submits = false;

    sim.open(0);
    sim.attach(0, 1, true);
    for by in [1, 10].into_iter().skip(given) {
        sim.send_from_editor(
            0,
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
    assert_eq!(count(&sim.content(0)), 111, "each edit landed once");
    assert_eq!(count(&sim.head_content()), 111);
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
