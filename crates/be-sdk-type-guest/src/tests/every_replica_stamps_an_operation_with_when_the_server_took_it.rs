use super::arrivals::*;
use super::*;

/// Every replica stamps a note with when the server took it, as the order says, so a replica
/// that joins later holds the same stamps; a note too large to submit inline, which goes by
/// its part, is stamped too.
#[test]
fn every_replica_stamps_an_operation_with_when_the_server_took_it() {
    let mut sim = Sim::<Arrivals>::new(3, arrivals_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(0, 1, Note::default());
    sim.settle();
    let first = sim.now as i64;
    sim.advance(500);
    let second = sim.now as i64;
    sim.operate(1, 1, Note::sized(2 * 1024 * 1024));
    sim.settle();
    for device in 0..2 {
        assert_eq!(
            entries(&mut sim, device),
            vec![Some(first), Some(second)],
            "device {device}"
        );
    }

    sim.open(2);
    sim.attach(2, 1, true);
    sim.settle();
    assert_eq!(
        entries(&mut sim, 2),
        vec![Some(first), Some(second)],
        "a replica that opened on the journal lacks a stamp"
    );
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
