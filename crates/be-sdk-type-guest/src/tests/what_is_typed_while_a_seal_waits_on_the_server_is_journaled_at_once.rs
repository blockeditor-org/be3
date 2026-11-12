use super::*;

/// An owner alone keeps journaling what is typed while its seal waits on a server that does not
/// answer, so the edit is on the device at once; once the seal lands, the journal starts over
/// from the commit holding what came after it, in one write, so a crash right then keeps it.
#[test]
fn what_is_typed_while_a_seal_waits_on_the_server_is_journaled_at_once() {
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 0, "soup");
    sim.settle();
    sim.stalled = true;
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.publishes, 0, "the seal waits on the server");

    sim.type_at(0, 1, 4, " and bread");
    sim.settle();
    sim.advance(crate::APPEND_INTERVAL);
    assert!(
        sim.devices[0].status.journaled,
        "what was typed during the seal is in the journal"
    );

    sim.unstall();
    assert_eq!(sim.publishes, 1);
    assert_eq!(prose(&sim.head_content()), "soup");
    let journal = sim.journal.clone().expect("the owner journals");
    assert_eq!(
        journal.base.commit, sim.head,
        "the journal starts over from the seal"
    );
    assert!(
        !journal.entries.is_empty(),
        "the journal still holds what came after the seal"
    );
    sim.crash(0);
    sim.open(0);
    assert_eq!(
        prose(&sim.content(0)),
        "soup and bread",
        "a crash right after the seal is answered keeps what came after it"
    );
    sim.attach(0, 1, true);
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(prose(&sim.head_content()), "soup and bread");
}
