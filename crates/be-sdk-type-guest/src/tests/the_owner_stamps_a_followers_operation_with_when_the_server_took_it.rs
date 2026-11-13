use super::arrivals::*;
use super::*;

/// The owner stamps a follower's note with when the server took it; the stamp is the op's own
/// content, so a replica that joins later (by a `Snapshot`) and a new owner after a handoff
/// hold it too. The owner's own note keeps none: its arrival is unknown.
#[test]
fn the_owner_stamps_a_followers_operation_with_when_the_server_took_it() {
    let mut sim = Sim::<Arrivals>::new(3, arrivals_module);
    sim.lease = Lease::lasting(0);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(0, 1, Note::default());
    sim.settle();
    sim.advance(500);
    let first = sim.now as i64;
    sim.operate(1, 1, Note::default());
    sim.settle();
    for device in 0..2 {
        assert_eq!(entries(&mut sim, device), vec![None, Some(first)], "device {device}");
    }

    sim.open(2);
    sim.attach(2, 1, true);
    sim.settle();
    assert_eq!(
        entries(&mut sim, 2),
        vec![None, Some(first)],
        "a replica that caught up by a snapshot lacks the stamp"
    );

    sim.claim(1);
    sim.settle();
    assert!(sim.devices[1].status.owner, "the lease was not handed over");
    sim.advance(500);
    let second = sim.now as i64;
    sim.operate(2, 1, Note::default());
    sim.settle();
    for device in 0..3 {
        assert_eq!(
            entries(&mut sim, device),
            vec![None, Some(first), Some(second)],
            "device {device}: the new owner stamps, and the earlier stamp stays"
        );
    }
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
