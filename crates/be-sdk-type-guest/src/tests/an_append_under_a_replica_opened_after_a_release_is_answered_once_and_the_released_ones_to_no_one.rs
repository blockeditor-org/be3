use super::*;

/// A replica released with an append unanswered carries no call to the block's next replica:
/// its editors are gone, and the answer goes to its id, which no later replica has. A replica
/// opened after the release, under a new id, answers its own editor's append once across a
/// reopen, and the released one's append is answered to no one.
#[test]
fn an_append_under_a_replica_opened_after_a_release_is_answered_once_and_the_released_ones_to_no_one() {
    let mut sim = Sim::<Logbook>::new(1, logbook_module);
    sim.keeps_modules = true;
    sim.open(0);
    sim.attach(0, 1, true);

    // An edit kept in the outbox, which the server has not taken, keeps the released replica's
    // carry, with an append the host has not answered.
    sim.kept_publishes = true;
    sim.offline_submits = true;
    sim.send_from_editor(0, 1, FromEditor::FollowLacking);
    sim.settle();
    sim.operate(0, 1, Counter::add(1));
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(sim.lacking(0, 1), 1, "the edit is kept in the outbox");
    sim.stalled_appends = true;
    let mirror = sim.editors.get_mut(&(0, 1)).unwrap();
    let (released, append) = mirror.append(b"before".to_vec(), be_block_content::Along::default());
    sim.send_from_editor(0, 1, append);
    sim.settle();
    sim.close(0);
    assert!(sim.devices[0].released, "the replica was released");

    // The block opens again in the module under a new replica, whose editor appends while it
    // opens again once more.
    sim.open_in_module(0);
    sim.attach(0, 2, true);
    let mirror = sim.editors.get_mut(&(0, 2)).unwrap();
    let (token, append) = mirror.append(b"after".to_vec(), be_block_content::Along::default());
    sim.send_from_editor(0, 2, append);
    sim.settle();
    sim.reopen(0);
    sim.take_received(0, 2);
    sim.unstall();

    let appended: Vec<_> = sim
        .take_received(0, 2)
        .into_iter()
        .filter(|message| matches!(message, ToEditor::Appended { .. }))
        .collect();
    assert!(
        matches!(appended.as_slice(), [ToEditor::Appended { token: answered, result: Ok(_) }] if *answered == token),
        "the new replica's editor hears its own append once, and no other: {appended:?}"
    );
    let told: Vec<_> = sim
        .take_received(0, 1)
        .into_iter()
        .filter(|message| matches!(message, ToEditor::Appended { token, .. } if *token == released))
        .collect();
    assert!(
        told.is_empty(),
        "the released replica's append is answered to no one: {told:?}"
    );
}
