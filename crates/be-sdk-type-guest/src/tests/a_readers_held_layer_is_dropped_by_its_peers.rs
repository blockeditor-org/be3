use super::*;
use be_block_session::SessionMessage;

/// A participant that may not edit holds nothing its peers preview: its `Held` is dropped.
#[test]
fn a_readers_held_layer_is_dropped_by_its_peers() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.devices[1].may_edit = false;
    sim.open(0);
    sim.attach(0, 1, true);
    sim.open(1);
    let (a, created) = Checklist::add("a");
    sim.operate(0, 1, created);
    sim.settle();
    sim.take_received(0, 1);

    let held = SessionMessage::Held {
        layer: 1,
        origin: sim.devices[1].origin,
        from: 0,
        to: 1,
        operations: vec![ChecklistContent::encode_operation(&Checklist::set_text(a, "read")).into()],
        commits_through: None,
    };
    let reader = sim.devices[1].client;
    sim.relay_as(0, reader, crate::wire::encode(&held));
    sim.settle();
    assert_eq!(texts(sim.mirror(0, 1)), ["a"]);
    assert!(sim.take_received(0, 1).is_empty());
}
