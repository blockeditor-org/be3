use super::*;

#[test]
fn typing_while_a_seal_waits_for_the_server_shows_at_once() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, add("first"));
    sim.settle();
    sim.stalled = true;
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(
        sim.held.len(),
        1,
        "the owner never asked to publish its seal"
    );

    sim.operate(0, 1, add("second"));
    sim.settle();
    sim.operate(0, 1, add("third"));
    sim.settle();
    assert_eq!(texts(sim.mirror(0, 1)), ["first", "second", "third"]);
    assert_eq!(texts(&sim.content(0)), ["first", "second", "third"]);
    assert_eq!(
        texts(sim.mirror(1, 1)),
        ["first"],
        "a follower heard of edits sequenced after a seal before the seal itself"
    );
    assert!(!sim.devices[0].status.clean);

    sim.unstall();
    assert_eq!(texts(&sim.head_content()), ["first"]);
    assert_eq!(texts(sim.mirror(1, 1)), ["first", "second", "third"]);
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(texts(&sim.head_content()), ["first", "second", "third"]);
    for device in 0..2 {
        assert_eq!(sim.devices[device].status.head, sim.head);
        assert_eq!(texts(sim.mirror(device, 1)), ["first", "second", "third"]);
    }
    assert!(sim.devices[0].status.clean);
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
