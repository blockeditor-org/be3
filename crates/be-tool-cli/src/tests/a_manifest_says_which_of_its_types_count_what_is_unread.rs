use super::*;

#[tokio::test]
async fn a_manifest_says_which_of_its_types_count_what_is_unread() {
    manifests();
    let _modules =
        be_block_type_host::started(be_type_bundle::sources(), be_block_client::Clock::real())
            .await;
    let mut counting = 0;
    for app in be_host_stack::discovery::manifests() {
        for block_type in &app.block_types {
            let content_type = Uuid::from_bytes(block_type.content_type());
            let reads = be_block_type_host::kind_of(content_type).and_then(|kind| kind.reads);
            assert_eq!(
                block_type.counts_unread,
                reads.is_some(),
                "{}'s {} says counts_unread {} in its manifest",
                app.identity.id,
                block_type.name,
                block_type.counts_unread,
            );
            counting += usize::from(block_type.counts_unread);
        }
    }
    assert!(counting > 0, "no block type counts what is unread");
}
