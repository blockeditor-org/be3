use super::*;

/// Two pastes over the limit, one after the other, each by its own part, land in order on
/// every replica, and a replica that opens after the first was put reads it too.
#[test]
fn two_oversized_edits_in_a_row_land_in_order() {
    let mut sim = Sim::<ProseContent>::new(3, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    let first = "a line of pasted text\n".repeat(60_000);
    let second = "another pasted line\n".repeat(60_000);
    sim.type_at(1, 1, 6, &first);
    sim.type_at(0, 1, 6, &second);
    sim.settle();
    assert_eq!(sim.parts_put, 2);
    sim.open(2);
    sim.attach(2, 1, true);
    sim.settle();
    let expected = sim.typed(0, 1);
    assert_eq!(expected.len(), 6 + first.len() + second.len());
    assert!(expected.starts_with("start\n"));
    for device in 0..3 {
        assert!(sim.typed(device, 1) == expected, "device {device}");
        assert!(prose(&sim.content(device)) == expected, "device {device}");
    }
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
