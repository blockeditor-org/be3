use super::*;

/// An editor that has not heard of two restarts (each reloads the content afresh, so nothing in
/// it keeps the identities its edits name) types on what it still shows: the replica carries the
/// edit from what the editor showed before the first, so it lands where it was typed. Once the
/// editor says it was shown the reloads (`FromEditor::Shown`), an edit of its that applies as
/// nothing (it erases what another editor erased first) is not carried: it erases nothing else.
#[test]
fn an_editors_edit_made_before_two_restarts_it_was_not_shown_lands() {
    crate::replica::RESTART_LIMIT.with(|limit| limit.set(64));
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.attach(0, 2, true);
    sim.type_at(0, 1, 0, "start\n");
    sim.advance(crate::QUIET_INTERVAL);

    sim.deafen(0, 1);
    for restart in 1..=2 {
        for round in 0..20 {
            sim.type_at(0, 2, 0, &format!("scratch {round} "));
            let typed = sim.typed(0, 2).find("start").expect("the text is there");
            sim.erase(0, 2, 0..typed);
        }
        // Settled first: edits still on their way at a restart do not all carry over it
        // (plans/backlog.md, "Edits on their way at a restart").
        sim.settle();
        sim.advance(crate::QUIET_INTERVAL);
        assert_eq!(
            sim.devices[0].status.restarts, restart,
            "the session restarted"
        );
    }
    assert_eq!(
        sim.typed(0, 1),
        "start\n",
        "editor 1 still shows the first content"
    );
    sim.type_at(0, 1, 5, "X");
    sim.settle();
    assert_eq!(
        prose(&sim.content(0)),
        "startX\n",
        "the edit lands where it was typed"
    );

    sim.hear(0, 1);
    assert!(
        sim.sent_from(0, 1)
            .contains(&FromEditor::Shown { reloads: 2 }),
        "editor 1 says it was shown both reloads"
    );
    assert_eq!(sim.typed(0, 1), "startX\n");
    sim.deafen(0, 1);
    sim.erase(0, 2, 2..5);
    sim.erase(0, 1, 2..5);
    sim.settle();
    assert_eq!(
        prose(&sim.content(0)),
        "stX\n",
        "an edit made after it was shown the reloads goes as made"
    );
}
