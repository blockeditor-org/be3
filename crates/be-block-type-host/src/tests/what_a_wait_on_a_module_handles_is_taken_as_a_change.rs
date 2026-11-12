use be_sdk_type_api::Change;

use super::*;

#[test]
fn what_a_wait_on_a_module_handles_is_taken_as_a_change() {
    with_test_types(async |peer, shared, mut replicas| {
        let block = block(&peer).await;
        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, false)
            .await;
        while replicas.status(block).is_none() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        replicas.pump((&peer, &shared)).await;
        assert!(
            !replicas.take_changed(),
            "the pump took every change the opening made"
        );
        let before = replicas.status(block).and_then(|status| status.head);
        let mut edited = base();
        edited.set(2, "deux");
        replicas.change_later(block, Change::Replace(edited.encode()));
        // The replica answers this after the status the change makes it report, so the wait
        // for that answer is what handles the status.
        replicas
            .content((&peer, &shared), block)
            .await
            .expect("the replica answers with its content");

        assert_ne!(
            replicas.status(block).and_then(|status| status.head),
            before,
            "the wait handled the status the change reported"
        );
        assert!(
            replicas.take_changed(),
            "what the wait handled is a change its caller is told of"
        );
        assert!(!replicas.take_changed(), "a change is told once");
    });
}
