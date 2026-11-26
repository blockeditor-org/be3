use super::*;
use be_block_session::SessionMessage;

/// A layer of a type whose edits do not absorb grows past what a peer shows: its holder tells
/// its peers to drop it, once, and tells them nothing more of it.
#[test]
fn a_layer_too_large_for_a_peer_to_show_is_told_no_more() {
    let mut sim = Sim::<Logbook>::new(2, logbook_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.settle();
    sim.gesture(0, 1, Gesture::BeginTentative);
    let mut told = Vec::new();
    for _ in 0..400 {
        sim.sent.clear();
        sim.operate(0, 1, Counter::add(1));
        sim.settle();
        told.push(
            sim.sent
                .iter()
                .filter_map(|(from, bytes)| match crate::wire::decode(bytes) {
                    Some(SessionMessage::Held { to, .. }) if *from == 0 => Some(to),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        );
    }
    let last = told.iter().rposition(|held| !held.is_empty()).unwrap();
    assert_eq!(told[last], [0], "the last it told was to drop it");
    assert!(last < 300, "told nothing after frame {last}");
    assert_eq!(sim.mirror(1, 1).counter.root().count, Count(0));
}
