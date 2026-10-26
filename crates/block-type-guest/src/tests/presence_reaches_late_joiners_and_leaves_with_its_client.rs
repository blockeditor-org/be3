use super::*;

const CURSOR: Uuid = Uuid::from_u128(0xc0);

fn shown(sim: &Sim<CounterContent>, device: usize) -> Vec<(ClientId, Vec<u8>)> {
    sim.editors[&(device, 1)]
        .presence()
        .iter()
        .map(|peer| (peer.client, peer.value.clone()))
        .collect()
}

#[test]
fn presence_reaches_late_joiners_and_leaves_with_its_client() {
    let mut sim = Sim::<CounterContent>::new(3, counter_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    let phone = sim.devices[0].client;
    sim.send_from_editor(
        0,
        1,
        FromEditor::ShowPresence {
            kind: CURSOR,
            value: Some(vec![1]),
        },
    );
    sim.settle();
    assert_eq!(shown(&sim, 1), [(phone, vec![1])]);
    assert!(shown(&sim, 0).is_empty(), "a peer never sees itself");

    sim.open(2);
    sim.attach(2, 1, true);
    assert_eq!(shown(&sim, 2), [(phone, vec![1])]);

    sim.crash(0);
    for device in 1..3 {
        assert!(
            shown(&sim, device).is_empty(),
            "the cursor of the phone that left stays"
        );
    }
}
