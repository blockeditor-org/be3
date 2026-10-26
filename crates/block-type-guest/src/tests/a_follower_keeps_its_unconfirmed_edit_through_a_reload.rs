use super::*;

#[test]
fn a_follower_keeps_its_unconfirmed_edit_through_a_reload() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.type_at(1, 1, 0, "mine ");
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(0, 1, 6, &pasted);
    sim.settle();
    let expected = format!("mine start\n{pasted}");
    for device in 0..2 {
        assert_eq!(prose(&sim.content(device)), expected);
        assert_eq!(sim.typed(device, 1), expected);
    }
    assert!(sim.devices[1].status.clean);
    assert!(!sim.devices[1].status.incompatible);
}
