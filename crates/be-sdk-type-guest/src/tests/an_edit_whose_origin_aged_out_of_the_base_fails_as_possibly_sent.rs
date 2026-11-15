use super::*;

/// The order numbered an edit while its replica heard nothing; then more devices than the base
/// names edited, so the base no longer names the deaf replica's origin. Opened again and given
/// the edit, the replica cannot tell whether the order holds it: it is not sent again, which
/// could make it land twice, and its editor is told it may have been sent.
#[test]
fn an_edit_whose_origin_aged_out_of_the_base_fails_as_possibly_sent() {
    let others = crate::wire::BaseState::KEPT_ORIGINS + 4;
    let mut sim = Sim::<CounterContent>::new(others + 2, counter_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(0, 1, Counter::add(1));
    sim.settle();

    sim.deaf.insert(1);
    sim.operate(1, 1, Counter::add(1000));
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
    let total = 1 + 1000 + others as i64;
    assert_eq!(count(&sim.head_content()), total);

    sim.deaf.clear();
    sim.kept_submitted = 1;
    sim.reopen(1);
    sim.send_from_editor(
        1,
        1,
        FromEditor::Operate(CounterContent::encode_operation(&Counter::add(1000))),
    );
    sim.settle();
    sim.follow_lacking(1, 1);
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    assert_eq!(count(&sim.head_content()), total, "the edit landed once");
    assert_eq!(count(&sim.content(1)), total);
    assert_eq!(
        sim.lacking(1, 1),
        1,
        "its editor is told it may have been sent"
    );
    assert!(
        sim.errors
            .iter()
            .any(|(device, error)| *device == 1 && error == crate::replica::UNCERTAIN_EDIT),
        "{:?}",
        sim.errors
    );
}
