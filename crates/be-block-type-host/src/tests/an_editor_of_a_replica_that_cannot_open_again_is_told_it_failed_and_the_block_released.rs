use be_block_content::BlockContent;
use be_type_text::TextContent;

use super::*;

/// A replica that ends to open again (as keys arrive, or as an unreadable block is reset) but
/// whose reopening fails tells its editors why, rather than leaving them on what it last showed,
/// and the host lets the block go and counts the replicas again, as it would for one that closed.
#[test]
fn an_editor_of_a_replica_that_cannot_open_again_is_told_it_failed_and_the_block_released() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = peer
            .create::<TextContent>(BlockParent::Root)
            .await
            .expect("the text is made");
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        replicas.pump((&peer, &shared)).await;
        let editor = 7;
        let mirror =
            attach_mirror::<TextContent>(&peer, &shared, &mut replicas, block, editor).await;
        assert!(mirror.is_loaded());
        shared.lock().unwrap().frames.clear();
        let counted = shared.lock().unwrap().counted;

        replicas
            .refuse_opening
            .insert(block, "the server refused it".to_owned());
        replicas.reopen(block);
        let released = |shared: &Arc<Mutex<Quiet>>| shared.lock().unwrap().released.contains(&block);
        let waited = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            while !released(&shared) {
                step(&peer, &shared, &mut replicas).await;
            }
        })
        .await;
        assert!(waited.is_ok(), "never let the block go");
        assert!(!replicas.is_open(block));
        assert!(
            shared.lock().unwrap().counted > counted,
            "the replicas were counted again once the ended one was gone"
        );
        let failed = std::mem::take(&mut shared.lock().unwrap().frames)
            .into_iter()
            .filter(|(to, _)| *to == editor)
            .filter_map(|(_, frame)| be_sdk_type_guest::editor::ToEditor::decode(&frame))
            .find_map(|message| match message {
                be_sdk_type_guest::editor::ToEditor::Failed(reason) => Some(reason),
                _ => None,
            });
        assert_eq!(failed.as_deref(), Some("the server refused it"));
    });
}
