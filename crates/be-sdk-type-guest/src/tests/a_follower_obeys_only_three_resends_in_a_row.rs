use be_block_session::SessionMessage;

use super::*;

/// A follower submits again at the owner's word three times, then reports its edit once and
/// stops answering; an edit of its own being accepted starts the count over.
#[test]
fn a_follower_obeys_only_three_resends_in_a_row() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    let owner = sim.devices[0].client;
    let resend = |sim: &mut Sim<ProseContent>| {
        sim.relay(
            1,
            owner,
            crate::wire::Wire::Session(SessionMessage::Resend).encode(),
        );
        sim.settle();
    };
    sim.deaf.insert(0);
    sim.type_at(1, 1, 6, "typed\n");
    sim.settle();
    for _ in 0..3 {
        resend(&mut sim);
    }
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
    resend(&mut sim);
    resend(&mut sim);
    assert_eq!(sim.errors.len(), 1, "reported once: {:?}", sim.errors);

    // Once the owner hears, its acceptance of the edit starts the count over.
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.deaf.insert(0);
    sim.type_at(1, 1, 6, "one\n");
    sim.settle();
    resend(&mut sim);
    resend(&mut sim);
    sim.deaf.clear();
    resend(&mut sim);
    assert!(
        sim.typed(0, 1) == "start\none\n",
        "the third resend delivered it"
    );
    sim.deaf.insert(0);
    sim.type_at(1, 1, 10, "two\n");
    sim.settle();
    for _ in 0..3 {
        resend(&mut sim);
    }
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
