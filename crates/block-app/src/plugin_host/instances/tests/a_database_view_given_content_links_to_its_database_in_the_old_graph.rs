use super::*;

use std::time::Duration;

use be_block::database_view::{DatabaseView, DatabaseViewContent};
use be_block::{BlockContent, BlockRef};
use block::Block;
use block_client::blocks::database_view::DatabaseView as ViewBlock;

fn seed(instances: &mut Instances, block: Uuid, database: Uuid) {
    instances.editor_message(EditorMessage::SeedContent {
        instance: INSTANCE,
        block_id: block.into_bytes(),
        content_type: DatabaseViewContent::CONTENT_TYPE.into_bytes(),
        bytes: DatabaseViewContent::new(&DatabaseView::of(BlockRef::Direct(database))).encode(),
    });
}

fn database_in(bytes: &[u8]) -> Option<Uuid> {
    DatabaseViewContent::decode(bytes)
        .ok()?
        .root()
        .database?
        .as_direct()
}

#[test]
fn a_database_view_given_content_links_to_its_database_in_the_old_graph() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let client = Arc::new(BlockClient::new(Uuid::nil(), Uuid::nil()));
    let view = client.create_block(ViewBlock::new());
    let (database, other) = (Uuid::new_v4(), Uuid::new_v4());
    let mut instances = placed_with(&client, view.id(), ViewBlock::TYPE_ID);

    seed(&mut instances, view.id(), database);
    instances.next_screens(PASS);
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        let held = shared.blocks.get(&view.id())?;
        (database_in(&held.bytes) == Some(database)).then_some(())
    })
    .expect("the view never took the content it was given");

    instances.next_screens(PASS);
    assert_eq!(view.read().unwrap().references(), [database]);

    seed(&mut instances, view.id(), other);
    crate::be::flush();
    let held = crate::be::content(view.id()).expect("the view is still open");
    assert_eq!(database_in(&held.bytes), Some(database));
}
