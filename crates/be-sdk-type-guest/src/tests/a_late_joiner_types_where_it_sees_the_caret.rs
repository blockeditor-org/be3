use super::*;

#[test]
fn a_late_joiner_types_where_it_sees_the_caret() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("one\n"));
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 4, "two\n");
    sim.advance(crate::SEAL_INTERVAL);
    sim.type_at(0, 1, 8, "three\n");
    sim.erase(0, 1, 0..4);
    sim.settle();
    sim.open(1);
    sim.attach(1, 1, true);
    assert_eq!(sim.typed(1, 1), "two\nthree\n");
    sim.type_at(1, 1, 4, "and a half\n");
    sim.settle();
    for device in 0..2 {
        assert_eq!(sim.typed(device, 1), "two\nand a half\nthree\n");
    }
    assert!(sim.devices[1].status.clean);
}
