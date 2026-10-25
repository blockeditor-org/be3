use super::*;

#[test]
fn two_peers_converge_while_the_lease_holder_hears_nothing() {
    let mut sim = Sim::<ProseContent>::new(3, prose_module);
    sim.publish_elsewhere(&prose_of("base\n"));
    for device in 0..3 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    assert!(sim.devices[0].status.owner);
    sim.deaf.insert(0);
    sim.stalled.insert(0);
    for round in 0..5 {
        sim.type_at(1, 1, 0, &format!("<{round}"));
        let end = sim.typed(2, 1).len();
        sim.type_at(2, 1, end, &format!("{round}>"));
        sim.settle();
    }
    let expected = sim.typed(1, 1);
    assert!(expected.contains("<4") && expected.contains("4>"));
    for device in 1..3 {
        assert_eq!(sim.typed(device, 1), expected);
        assert_eq!(prose(&sim.content(device)), expected);
        assert!(
            sim.devices[device].status.clean,
            "an edit waited on the lease holder"
        );
    }
    assert_eq!(sim.typed(0, 1), "base\n", "the holder heard nothing");

    sim.deaf.remove(&0);
    sim.unstall(0);
    sim.resynchronize(0);
    assert_eq!(sim.typed(0, 1), expected);
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(prose(&sim.head_content()), expected);
}
