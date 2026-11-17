use super::*;

/// The order restarts with an edit on its way, and its replica submits it again carried over;
/// before the host hears that submission ordered, the type's module stops and starts again. The
/// replica opened in its place is told the restart its edits and kept submissions were made
/// under (`opening.kept-restart`), not the one the block was first opened with, so it takes none
/// of them as made before a restart, and the edit lands once.
#[test]
fn a_replica_opened_again_after_a_restart_is_told_it_and_its_edit_lands_once() {
    with_test_types(|peer, shared, mut replicas| async move {
        let (block, _) = carry_an_edit_over_a_restart(&peer, &shared, &mut replicas).await;

        replicas.modules.restart_type(Tally::CONTENT_TYPE);
        pump_until(&peer, &shared, &mut replicas, "reopened the replica", |replicas| {
            replicas
                .submissions_of(block)
                .is_some_and(|(_, restarts, _)| restarts == 0)
        })
        .await;
        let (_, kept_restart, _) = replicas
            .openings
            .last()
            .cloned()
            .expect("the replica was opened again");
        assert_eq!(
            kept_restart, 1,
            "the replica opened again is told the restart what it is given was made under"
        );
        step_until_replicas(&peer, &shared, &mut replicas, "settled the edit", |replicas| {
            replicas
                .submissions_of(block)
                .is_some_and(|(submitting, restarts, _)| submitting == 0 && restarts >= 1)
        })
        .await;
        for _ in 0..3 {
            step(&peer, &shared, &mut replicas).await;
        }
        let bytes = replicas
            .content((&peer, &shared), block)
            .await
            .expect("the tally is open");
        let tally = Tally::decode(&bytes).expect("the tally decodes");
        assert_eq!(tally.count, 10, "the edit landed once");
        let errors = std::mem::take(&mut shared.lock().unwrap().errors);
        assert!(errors.is_empty(), "{errors:?}");
    });
}
