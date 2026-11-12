use super::*;

/// With others in the session the owner journals what is typed while its seal is on its way,
/// as when alone. A follower that was there and one that joined meanwhile both applied it
/// before the seal lands: they load the seal and hear what came after it again, and every
/// device ends with the same text, published once.
#[test]
fn followers_keep_what_the_owner_typed_while_its_seal_was_out() {
    let mut sim = Sim::<ProseContent>::new(3, prose_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.type_at(0, 1, 0, "soup");
    sim.settle();
    sim.stalled = true;
    sim.advance(crate::QUIET_INTERVAL);
    sim.type_at(0, 1, 4, " and bread");
    sim.settle();
    sim.advance(crate::APPEND_INTERVAL);
    assert!(
        sim.devices[0].status.journaled,
        "what was typed during the seal is in the journal"
    );
    sim.open(2);
    sim.attach(2, 1, true);

    sim.unstall();
    sim.advance(crate::QUIET_INTERVAL);
    for device in 0..3 {
        assert_eq!(sim.typed(device, 1), "soup and bread", "device {device}");
    }
    assert_eq!(prose(&sim.head_content()), "soup and bread");
    sim.type_at(1, 1, 0, "> ");
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    for device in 0..3 {
        assert_eq!(sim.typed(device, 1), "> soup and bread", "device {device}");
    }
}
