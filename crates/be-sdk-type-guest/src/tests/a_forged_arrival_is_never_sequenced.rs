use super::arrivals::*;
use super::*;

use be_block_session::Payload;

/// Only the owner stamps an arrival. A note that already claims one is dropped, inline or by
/// its part, and reported as incompatible, and a replica refuses to make one, so an honest
/// follower never sends it.
#[test]
fn a_forged_arrival_is_never_sequenced() {
    let mut sim = Sim::<Arrivals>::new(2, arrivals_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    let follower = sim.devices[1].client;
    let forged = Note {
        stamp: Some(1),
        padding: Vec::new(),
    };

    let inline = Payload::Inline(Arrivals::encode_operation(&forged));
    sim.relay_as(
        0,
        follower,
        RelayKind::Edit,
        submit(follower, 1_000, inline),
    );
    let large = Note {
        stamp: Some(1),
        padding: vec![0; 2 * 1024 * 1024],
    };
    let manifest = sim
        .vault
        .write(
            SEALING,
            Uuid::nil(),
            be_block_content::Part::from(Arrivals::encode_operation(&large).as_slice()),
        )
        .unwrap();
    let part = Payload::Part(manifest.root.expect("the part has bytes"));
    sim.relay_as(0, follower, RelayKind::Edit, submit(follower, 1_001, part));
    sim.settle();
    assert_eq!(
        entries(&mut sim, 0),
        Vec::<Option<i64>>::new(),
        "a forged stamp was sequenced"
    );
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
    assert!(
        sim.devices[0].status.incompatible,
        "the owner did not report the forgery it dropped"
    );

    sim.operate(1, 1, forged);
    sim.settle();
    assert_eq!(
        sim.errors
            .iter()
            .map(|(device, _)| *device)
            .collect::<Vec<_>>(),
        vec![1],
        "the follower's replica took an edit that claims its arrival"
    );
    for device in 0..2 {
        assert_eq!(
            entries(&mut sim, device),
            Vec::<Option<i64>>::new(),
            "device {device}"
        );
    }

    sim.operate(1, 1, Note::default());
    sim.settle();
    let now = sim.now as i64;
    for device in 0..2 {
        assert_eq!(
            entries(&mut sim, device),
            vec![Some(now)],
            "device {device}: the follower's next note was held back"
        );
    }
}
