use super::*;

#[test]
fn a_replacement_made_while_the_owner_is_gone_reaches_every_peer() {
    let mut sim = Sim::<ChecklistContent>::new(3, checklist_module);
    for device in 0..3 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(2, 1, add("unsealed"));
    sim.settle();
    sim.deaf.insert(0);
    let mut replacement = ChecklistContent::default();
    replacement.apply(&add("replacement"));
    sim.send_from_editor(1, 1, FromEditor::Replace(replacement.encode()));
    sim.settle();
    sim.crash(0);
    sim.deaf.clear();
    sim.advance(crate::SEAL_INTERVAL);
    assert!(sim.devices[1].status.owner);
    assert_eq!(texts(&sim.head_content()), ["replacement"]);
    for device in 1..3 {
        assert_eq!(texts(&sim.content(device)), ["replacement"]);
        assert_eq!(texts(sim.mirror(device, 1)), ["replacement"]);
        assert_eq!(sim.devices[device].status.head, sim.head);
    }
    assert!(sim.kept(|content| texts(content).contains(&"unsealed".to_owned())));
    sim.operate(2, 1, add("afterwards"));
    sim.settle();
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(texts(&sim.head_content()), ["replacement", "afterwards"]);
}
