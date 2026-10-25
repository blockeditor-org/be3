use super::*;

#[test]
fn a_peer_running_another_build_is_refused() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.devices[1].tag = SessionTag([9; 32]);
    sim.open(0);
    sim.open(1);
    assert!(sim.devices[1].status.incompatible);
    assert!(sim.devices[1].status.diverged);
    assert!(!sim.devices[0].status.incompatible);
    sim.attach(1, 1, true);
    sim.operate(1, 1, add("refused"));
    sim.settle();
    assert!(texts(&sim.content(0)).is_empty());
    assert!(!sim.errors.is_empty());
}
