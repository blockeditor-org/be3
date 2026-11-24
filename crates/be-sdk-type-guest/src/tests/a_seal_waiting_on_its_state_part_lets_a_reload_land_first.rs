use super::*;

/// The holder's seal waits for the part its session state goes by while a peer's replacement
/// reaches the order. The replacement lands on the peer; on the holder it waits behind the seal,
/// which goes on once the part is put, and lands after it: nothing either waited for is lost,
/// and the session seals what it holds.
#[test]
fn a_seal_waiting_on_its_state_part_lets_a_reload_land_first() {
    crate::replica::LARGEST_INLINE.with(|largest| largest.set(64));
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.type_at(0, 1, 0, "start\n");
    for round in 0..40 {
        sim.type_at(0, 1, 0, &format!("scratch {round} "));
        let typed = sim.typed(0, 1).find("start").expect("the text is there");
        sim.erase(0, 1, 0..typed);
    }
    sim.settle();

    // The seal puts its state, and the put is held.
    sim.stalled = true;
    sim.advance(crate::QUIET_INTERVAL);
    assert!(
        sim.held
            .iter()
            .any(|(device, call)| *device == 0 && matches!(call, Call::PutPart { .. })),
        "the seal waits for its state's part"
    );
    sim.stalled = false;
    assert!(sim.change(1, Change::Replace(prose_of("replaced\n").encode())));
    sim.settle();
    assert_eq!(
        prose(&sim.content(1)),
        "replaced\n",
        "the replacement landed"
    );

    // The part is put: the seal goes on, then the replacement lands on the holder too, and the
    // session seals what it holds.
    let held = std::mem::take(&mut sim.held);
    for (device, call) in held {
        sim.act(device, call);
    }
    sim.settle();
    assert_eq!(prose(&sim.content(0)), "replaced\n");
    sim.advance(crate::QUIET_INTERVAL);
    sim.settle();
    assert_eq!(prose(&sim.head_content()), "replaced\n");
    assert!(!sim.devices[0].status.owes_seal, "nothing is left to seal");
    sim.type_at(1, 1, 0, "more ");
    sim.advance(crate::QUIET_INTERVAL);
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(
        prose(&sim.head_content()),
        "more replaced\n",
        "and it seals again"
    );
}
