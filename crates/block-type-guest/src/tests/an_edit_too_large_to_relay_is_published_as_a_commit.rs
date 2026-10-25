use super::*;

#[test]
fn an_edit_too_large_to_relay_is_published_as_a_commit() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(0, 1, 6, &pasted);
    sim.settle();
    assert_eq!(sim.publishes, 1, "the large edit is published");
    assert!(
        sim.largest_relay < 1024 * 1024,
        "a relayed message carried {} bytes",
        sim.largest_relay
    );
    let expected = format!("start\n{pasted}");
    assert_eq!(prose(&sim.head_content()), expected);
    assert_eq!(sim.typed(1, 1), expected);
    sim.type_at(0, 1, 0, "> ");
    sim.settle();
    assert!(sim.typed(1, 1).starts_with("> start"));
}
