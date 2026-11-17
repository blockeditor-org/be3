use super::*;

/// A device types an edit the server never takes, and stops. Meanwhile the session restarts
/// from its saved content elsewhere, which moves what the edit was made on. Opened again, the
/// device is given the edit it kept: it is not applied, and its editor is told it was not sent.
#[test]
fn an_edit_kept_across_a_stop_made_before_a_restart_since_is_told_not_sent() {
    crate::replica::RESTART_LIMIT.with(|limit| limit.set(8));
    let mut sim = Sim::<ProseContent>::new(2, prose_with_undo_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);

    sim.stalled_submits = true;
    let client = sim.devices[1].client;
    let op = sim
        .mirror(1, 1)
        .text(ObjectId::ROOT, Prose::BODY)
        .and_then(|body| body.insert(client, 0, b"kept ".to_vec()))
        .expect("there is somewhere to type");
    let kept = FromEditor::Operate { gesture: None, operation: ProseContent::encode_operation(
        &Prose::BODY.edit(ObjectId::ROOT, op).into(),
    ) };
    sim.send_from_editor(1, 1, kept.clone());
    sim.settle();
    let kept_restart = sim.devices[1].status.restarts;
    sim.held.clear();
    sim.stalled_submits = false;
    sim.crash(1);

    for round in 0..20 {
        sim.type_at(0, 1, 0, &format!("scratch {round} "));
        let typed = sim.typed(0, 1).find("start").unwrap();
        sim.erase(0, 1, 0..typed);
    }
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    sim.settle();
    assert!(
        sim.devices[0].status.restarts > kept_restart,
        "the session restarted while the device was away"
    );

    sim.kept_opening = Some((kept_restart, 1));
    sim.open(1);
    sim.attach(1, 1, true);
    sim.send_from_editor(1, 1, kept);
    sim.settle();
    sim.follow_lacking(1, 1);
    assert_eq!(
        prose(&sim.content(1)),
        "start\n",
        "the kept edit was not applied"
    );
    assert_eq!(sim.lacking(1, 1), 1, "its editor is told it was not sent");
    assert!(
        sim.errors
            .iter()
            .any(|(device, error)| *device == 1 && error == crate::replica::STALE_EDIT),
        "{:?}",
        sim.errors
    );
}
