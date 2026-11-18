use super::*;

/// A device that numbers its replica's edit offline in an order behind the one the replica
/// applied (it kept no copy of the journal the replica opened on) hands the edit back at a place
/// the replica passed: it is not applied, and would wait for good, so the replica says so.
#[test]
fn an_own_edit_numbered_behind_what_the_replica_applied_is_an_error() {
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 0, "soup");
    sim.settle();
    assert!(sim.errors.is_empty(), "a replay of what it applied is quiet");

    sim.offline_submits = true;
    sim.offline_order = Some(Sequencer::starting(0));
    sim.type_at(0, 1, 4, " and bread");
    sim.settle();
    assert!(
        sim.errors
            .iter()
            .any(|(device, error)| *device == 0 && error.contains("behind")),
        "the replica says its own edit came back behind it: {:?}",
        sim.errors
    );
}
