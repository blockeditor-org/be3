use super::*;

#[test]
fn a_follower_that_takes_over_saves_its_edit_too_large_to_relay() {
    let mut sim = Sim::<ProseContent>::new(3, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..3 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.deaf.insert(0);
    sim.type_at(1, 1, 6, "typed\n");
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(1, 1, 12, &pasted);
    sim.settle();
    sim.crash(0);
    sim.deaf.clear();
    sim.advance(crate::SEAL_INTERVAL);
    assert!(sim.devices[1].status.owner);
    assert!(
        sim.largest_relay < 1024 * 1024,
        "a relayed message carried {} bytes",
        sim.largest_relay
    );
    let expected = format!("start\ntyped\n{pasted}");
    assert!(prose(&sim.head_content()) == expected);
    for device in 1..3 {
        assert!(prose(&sim.content(device)) == expected);
        assert!(sim.typed(device, 1) == expected);
    }
}
