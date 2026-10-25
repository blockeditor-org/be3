use super::*;

#[test]
fn a_late_joiner_catches_up_from_the_server_without_the_lease_holder() {
    let mut sim = Sim::<ChecklistContent>::new(3, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.operate(0, 1, add("sealed"));
    sim.settle();
    sim.advance(crate::SEAL_INTERVAL);
    sim.operate(0, 1, add("unsealed"));
    sim.settle();
    sim.deaf.insert(0);

    sim.open(1);
    sim.attach(1, 1, true);
    assert_eq!(texts(sim.mirror(1, 1)), ["sealed", "unsealed"]);
    sim.operate(1, 1, add("joined"));
    sim.settle();
    sim.open(2);
    sim.attach(2, 1, true);
    assert_eq!(
        texts(sim.mirror(2, 1)),
        ["sealed", "unsealed", "joined"],
        "the late joiner waited on a lease holder that hears nothing"
    );
    assert!(sim.devices[0].status.owner);
}
