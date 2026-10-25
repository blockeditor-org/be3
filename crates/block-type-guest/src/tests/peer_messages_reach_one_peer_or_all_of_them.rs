use super::*;

const CHANNEL: Uuid = Uuid::from_u128(0xc4a);

fn heard(sim: &mut Sim<CounterContent>, device: usize) -> Vec<Vec<u8>> {
    let phone = sim.devices[0].client;
    sim.editors
        .get_mut(&(device, 1))
        .unwrap()
        .take_peer_messages()
        .into_iter()
        .map(|message| {
            assert_eq!(message.from, phone);
            assert_eq!(message.channel, CHANNEL);
            message.payload
        })
        .collect()
}

#[test]
fn peer_messages_reach_one_peer_or_all_of_them() {
    let mut sim = Sim::<CounterContent>::new(3, counter_module);
    for device in 0..3 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    let laptop = sim.devices[1].client;
    for (to, payload) in [(Some(laptop), "to the laptop"), (None, "to everyone")] {
        sim.send_from_editor(
            0,
            1,
            FromEditor::SendToPeers {
                to,
                channel: CHANNEL,
                payload: payload.into(),
            },
        );
    }
    sim.settle();
    assert_eq!(
        heard(&mut sim, 1),
        [b"to the laptop".to_vec(), b"to everyone".to_vec()],
        "a peer hears what was sent to it in the order it was sent"
    );
    assert_eq!(heard(&mut sim, 2), [b"to everyone".to_vec()]);
    assert!(heard(&mut sim, 0).is_empty(), "a peer never hears itself");
}
