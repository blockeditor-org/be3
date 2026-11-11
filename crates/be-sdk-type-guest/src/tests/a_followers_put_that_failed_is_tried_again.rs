use super::*;

/// A follower whose put of an edit's part fails keeps the edit and puts it again after
/// `PUT_RETRY_INTERVAL`, with nothing else to prompt it.
#[test]
fn a_followers_put_that_failed_is_tried_again() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.fail_puts = 2;
    let pasted = "a line of pasted text\n".repeat(60_000);
    sim.type_at(1, 1, 6, &pasted);
    sim.settle();
    assert_eq!(sim.parts_put, 1);
    assert_eq!(sim.errors.len(), 1, "the failure is reported");
    assert!(sim.typed(0, 1) == "start\n", "nothing reached the owner");
    let expected = format!("start\n{pasted}");
    assert!(sim.typed(1, 1) == expected, "the follower keeps its edit");

    sim.advance(5_000);
    assert_eq!(sim.parts_put, 2, "the put was tried again");
    assert!(sim.typed(0, 1) == "start\n", "and failed again");
    sim.advance(5_000);
    assert_eq!(sim.parts_put, 2, "the second wait is longer");
    sim.advance(5_000);
    assert_eq!(sim.parts_put, 3);
    for device in 0..2 {
        assert!(prose(&sim.content(device)) == expected, "device {device}");
        assert!(sim.typed(device, 1) == expected, "device {device}");
    }
    assert_eq!(sim.errors.len(), 2, "each failure is reported");
}
