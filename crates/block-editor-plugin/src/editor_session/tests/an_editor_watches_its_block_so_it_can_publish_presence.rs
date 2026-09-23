use super::*;
use block::Block;
use block_client::blocks::counter::Counter;

struct BlockIgnoringApp;

impl crate::BeuiApp for BlockIgnoringApp {
    fn view(_editor: crate::Editor) -> beui::NodeId {
        beui::reactive::Frame().build()
    }
}

#[test]
fn an_editor_watches_its_block_so_it_can_publish_presence() {
    let mut session = EditorSession::new::<BlockIgnoringApp>(EditorInstanceId(0), Waker::default());
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block_id = Uuid::new_v4();

    session.connect(Arc::clone(&client), block_id, Counter::TYPE_ID);

    assert!(
        client.watches_block(block_id),
        "an editor must watch its own block, or the server rejects the presence it posts for it"
    );
    assert!(session.block.is_some());
}
