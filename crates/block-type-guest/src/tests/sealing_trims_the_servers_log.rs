use super::*;

#[test]
fn sealing_trims_the_servers_log() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    for text in ["a", "b", "c"] {
        sim.operate(0, 1, add(text));
    }
    sim.settle();
    assert!(sim.held() > 0, "the server held nothing it had not sealed");
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(
        sim.held(),
        0,
        "the server kept what the lease holder sealed"
    );
    assert_eq!(
        sim.sequencer.as_ref().unwrap().seal().head,
        sim.head,
        "the server serves the seal it was told of"
    );
    sim.operate(0, 1, add("d"));
    sim.settle();
    assert_eq!(sim.held(), 1);

    sim.open(1);
    sim.attach(1, 1, true);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "b", "c", "d"]);
}
