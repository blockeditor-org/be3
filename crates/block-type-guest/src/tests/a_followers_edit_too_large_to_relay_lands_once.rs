use super::*;

#[test]
fn a_followers_edit_too_large_to_relay_lands_once() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.type_at(1, 1, 6, "typed\n");
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(1, 1, 12, &pasted);
    sim.settle();
    sim.advance(crate::SEAL_INTERVAL);
    assert!(
        sim.largest_relay < 1024 * 1024,
        "a relayed message carried {} bytes",
        sim.largest_relay
    );
    let expected = format!("start\ntyped\n{pasted}");
    assert!(prose(&sim.head_content()) == expected);
    for device in 0..2 {
        assert!(prose(&sim.content(device)) == expected);
        assert!(sim.typed(device, 1) == expected);
    }
    assert!(sim.conflicts.is_empty());
    sim.type_at(1, 1, 0, "> ");
    sim.settle();
    sim.type_at(0, 1, 2, "!");
    sim.settle();
    sim.advance(crate::SEAL_INTERVAL);
    let expected = format!("> !start\ntyped\n{pasted}");
    assert!(prose(&sim.head_content()) == expected);
    for device in 0..2 {
        assert!(sim.typed(device, 1) == expected);
    }
}
