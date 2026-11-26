use super::*;

/// An edit is on its way when the session restarts: the server numbers it after the restart,
/// every replica passes it over, and its replica submits it again carried over, which rewrites
/// where it goes. That submission is kept, and the replica is opened again (as after a crash)
/// before the server numbers it. The host gives the edit again as typed, which does not encode
/// as the carried one: matched by the edits it was carried from, the kept submission goes again
/// as it was, under its id, so when the first copy reaches the server after it, the edit lands
/// once.
#[test]
fn an_edit_carried_over_a_restart_given_again_as_typed_goes_under_its_kept_id() {
    crate::replica::RESTART_LIMIT.with(|limit| limit.set(8));
    let mut sim = Sim::<ProseContent>::new(2, prose_with_undo_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    for round in 0..20 {
        sim.type_at(0, 1, 0, &format!("scratch {round} "));
        let typed = sim.typed(0, 1).find("start").unwrap();
        sim.erase(0, 1, 0..typed);
    }
    sim.settle();

    // Typed inside text the holder erases next, as the editor sends it, which the host keeps
    // to give again: once the restart drops what was erased, carrying it over moves it.
    sim.type_at(0, 1, 0, "scratch ");
    sim.settle();
    let client = Sim::<ProseContent>::editor_client(1, 1);
    let op = sim
        .mirror(1, 1)
        .text(ObjectId::ROOT, Prose::BODY)
        .and_then(|body| body.insert(client, 10, b"pending ".to_vec()))
        .expect("there is somewhere to type");
    let as_typed = ProseContent::encode_operation(&Prose::BODY.edit(ObjectId::ROOT, op).into());
    let typed = FromEditor::Operate {
        gesture: None,
        operation: as_typed.clone(),
    };
    // Its submissions are held on their way: the first, as typed, and the one carried over the
    // restart that comes while the first is on its way.
    sim.stalled_from = Some(1);
    sim.send_from_editor(1, 1, typed.clone());
    sim.settle();
    let scratch = sim.typed(0, 1).find("start").unwrap();
    sim.erase(0, 1, 0..scratch);
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    sim.settle();
    let held: Vec<be_sdk_type_api::Submission> = std::mem::take(&mut sim.held)
        .into_iter()
        .filter_map(|(device, call)| match call {
            Call::Submit { submission, .. } if device == 1 => Some(submission),
            _ => None,
        })
        .collect();
    sim.stalled_from = None;
    let [first, carried] = <[_; 2]>::try_from(held).expect("submitted as typed and carried over");
    let restarts = sim.devices[1].status.restarts;
    assert!(restarts > 0, "the session restarted");
    let held = Submitted::decode(&carried.payload).expect("a submission decodes");
    assert!(
        matches!(&held.entry, Entry::Operation(be_block_session::Payload::Inline(bytes))
            if *bytes != as_typed),
        "carrying it over rewrote the edit"
    );

    sim.kept_opening = Some((restarts, 0));
    sim.kept_submitted = held.through;
    sim.kept_submissions = vec![be_sdk_type_api::KeptSubmission {
        counter: carried.counter,
        through: held.through,
        payload: carried.payload.clone(),
    }];
    // The submissions taken on their way are never answered: the replica they went from is
    // opened again.
    sim.given_again = vec![(1, typed)];
    sim.errors.clear();
    sim.reopen(1);
    sim.settle();
    // Both copies the crashed process sent reach the server after the one given again: the
    // first, as typed, is passed over as made before the restart.
    sim.number(1, first);
    sim.number(1, carried);
    sim.settle();
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    assert_eq!(
        prose(&sim.head_content()),
        "stpending art\n",
        "the edit landed once"
    );
    for device in 0..2 {
        assert_eq!(
            prose(&sim.content(device)),
            "stpending art\n",
            "device {device}"
        );
    }
    assert!(sim.devices[1].status.clean, "the edit settled");
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
