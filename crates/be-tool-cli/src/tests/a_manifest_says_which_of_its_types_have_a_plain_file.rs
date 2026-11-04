use super::*;

#[tokio::test]
async fn a_manifest_says_which_of_its_types_have_a_plain_file() {
    manifests();
    let _modules =
        be_block_type_host::started(be_type_bundle::sources(), be_block_client::Clock::real())
            .await;
    let mut checked = 0;
    for app in crate::apps::installed() {
        for block_type in &app.manifest.block_types {
            let content_type = Uuid::from_bytes(block_type.content_type());
            assert_eq!(
                block_type.descriptor.plain_file,
                be_block_type_host::plain_of(content_type) == be_block_type_host::Plain::File,
                "{}'s {} says plain_file {} in its manifest",
                app.manifest.identity.id,
                block_type.name,
                block_type.descriptor.plain_file,
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no block types were checked");
}
