use super::*;

/// A read of an operation's part that fails is tried again, and the follower does not
/// diverge: what the owner sequenced after the paste waits behind it.
#[test]
fn a_follower_reads_a_part_again_when_a_read_fails() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.fail_reads = 1;
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(0, 1, 6, &pasted);
    sim.type_at(0, 1, 0, "> ");
    sim.settle();
    assert!(sim.typed(1, 1) == "start\n", "the follower still waits");
    sim.advance(1_000);
    let expected = format!("> start\n{pasted}");
    for device in 0..2 {
        assert!(prose(&sim.content(device)) == expected, "device {device}");
        assert!(sim.typed(device, 1) == expected, "device {device}");
    }
    assert_eq!(sim.part_reads, 2, "the part was read again");
    sim.type_at(1, 1, 2, "!");
    sim.settle();
    assert!(
        sim.typed(0, 1).starts_with("> !start"),
        "the follower still edits"
    );
}
