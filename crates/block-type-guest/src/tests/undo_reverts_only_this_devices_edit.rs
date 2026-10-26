use super::*;

#[test]
fn undo_reverts_only_this_devices_edit() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, add("mine"));
    sim.advance(1_000);
    sim.operate(1, 1, add("theirs"));
    sim.settle();
    assert_eq!(sim.devices[0].history, (true, false));
    assert!(sim.change(0, Change::Undo));
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["theirs"]);
        assert_eq!(texts(sim.mirror(device, 1)), ["theirs"]);
    }
    assert_eq!(sim.devices[0].history, (false, true));
    assert!(sim.change(0, Change::Redo));
    let mut both = texts(&sim.content(1));
    both.sort();
    assert_eq!(both, ["mine", "theirs"]);
    assert!(!sim.change(0, Change::Redo));
}
