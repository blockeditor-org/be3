use super::*;

#[test]
fn a_requested_seal_saves_edits_still_on_their_way() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.stalled.insert(0);
    sim.operate(0, 1, Counter::add(5));
    sim.settle();
    assert!(!sim.devices[0].status.clean);
    assert_eq!(
        sim.request(
            0,
            Request::Seal {
                replica: REPLICA,
                force: false,
            },
        ),
        Reply::Sealed
    );
    assert_eq!(count(&sim.head_content()), 5, "the seal left the edit out");

    sim.unstall(0);
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(count(&sim.head_content()), 5, "the edit was saved twice");
    for device in 0..2 {
        assert_eq!(count(&sim.content(device)), 5);
        assert!(sim.devices[device].status.clean);
    }
}
