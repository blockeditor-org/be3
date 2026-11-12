use super::*;

/// The server takes the seal, but the device stops before it hears so: its journal is still on
/// the commit the seal was made on. Opening at the sealed head replays only what came after the
/// sequence the seal holds through, so what was typed meanwhile is kept, once.
#[test]
fn what_is_typed_while_a_seal_waits_survives_a_crash_before_the_seal_is_answered() {
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 0, "soup");
    sim.settle();
    sim.stalled = true;
    sim.advance(crate::QUIET_INTERVAL);
    sim.type_at(0, 1, 4, " and bread");
    sim.settle();
    sim.advance(crate::APPEND_INTERVAL);
    sim.crash(0);
    sim.unstall();
    assert_eq!(
        prose(&sim.head_content()),
        "soup",
        "the server took the seal"
    );

    sim.open(0);
    assert_eq!(prose(&sim.content(0)), "soup and bread");
    sim.attach(0, 1, true);
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(prose(&sim.head_content()), "soup and bread");
}
