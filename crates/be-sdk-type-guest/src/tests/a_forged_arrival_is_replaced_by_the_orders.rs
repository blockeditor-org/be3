use super::arrivals::*;
use super::*;

/// Only the order stamps an arrival. A replica refuses to make an edit that claims one, and
/// every replica stamps what the order numbered with when the server took it, whatever its
/// bytes say, so a forged stamp is never held.
#[test]
fn a_forged_arrival_is_replaced_by_the_orders() {
    let mut sim = Sim::<Arrivals>::new(2, arrivals_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(
        1,
        1,
        Note {
            stamp: Some(1),
            padding: Vec::new(),
        },
    );
    sim.settle();
    assert_eq!(
        sim.errors
            .iter()
            .map(|(device, _)| *device)
            .collect::<Vec<_>>(),
        vec![1],
        "the replica took an edit that claims its arrival"
    );
    for device in 0..2 {
        assert_eq!(
            entries(&mut sim, device),
            Vec::<Option<i64>>::new(),
            "device {device}"
        );
    }

    // A forger's submission, its stamp claimed: the order's time replaces it everywhere.
    sim.advance(250);
    let now = sim.now as i64;
    sim.submit_as(
        1,
        Submitted {
            through: 0,
            entry: Entry::Operation(be_block_session::Payload::Inline(
                Arrivals::encode_operation(&Note {
                    stamp: Some(1),
                    padding: Vec::new(),
                }),
            )),
        }
        .encode(),
    );
    for device in 0..2 {
        assert_eq!(
            entries(&mut sim, device),
            vec![Some(now)],
            "device {device}"
        );
    }
}
