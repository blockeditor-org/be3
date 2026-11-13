use super::*;

/// A module started on the way to opening a block, not through `Replicas::kind`, registers its
/// kinds from its hello: the host hears it (`Host::kinds_changed`), so a block of that type
/// stops being stale.
#[test]
fn a_module_started_by_an_open_tells_the_host_of_its_kinds() {
    with_test_types(async |peer, shared, mut replicas| {
        let before = KINDS_CHANGED.load(std::sync::atomic::Ordering::Relaxed);
        let block = block(&peer).await;
        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, false)
            .await;
        while replicas.status(block).is_none() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        assert!(crate::is_known(Sections::CONTENT_TYPE));
        assert!(
            KINDS_CHANGED.load(std::sync::atomic::Ordering::Relaxed) > before,
            "the host heard of the kinds the module registered"
        );
    });
}
