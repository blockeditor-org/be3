use super::*;

/// The owner puts its paste as a part and sequences it once the part is put, holding back what
/// it types meanwhile; a follower holds back what was sequenced after the paste until it has
/// read the part.
#[test]
fn an_owners_edit_too_large_to_relay_goes_by_its_part_and_is_committed_at_once() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(0, 1, 6, &pasted);
    sim.type_at(0, 1, 0, "> ");
    sim.settle();
    assert_eq!(sim.parts_put, 1, "the owner put the edit as a part");
    assert_eq!(sim.part_reads, 1, "the follower read it");
    assert!(
        sim.largest_relay < 1024 * 1024,
        "a relayed message carried {} bytes",
        sim.largest_relay
    );
    let expected = format!("> start\n{pasted}");
    for device in 0..2 {
        assert!(prose(&sim.content(device)) == expected, "device {device}");
        assert!(sim.typed(device, 1) == expected, "device {device}");
    }
    sim.advance(0);
    assert_eq!(sim.publishes, 1, "the owner committed at once");
    assert!(prose(&sim.head_content()) == expected);
    sim.type_at(1, 1, 2, "!");
    sim.settle();
    assert!(sim.typed(0, 1).starts_with("> !start"));
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
