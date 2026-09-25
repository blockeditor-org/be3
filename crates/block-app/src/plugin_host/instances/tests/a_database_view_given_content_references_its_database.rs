use super::*;
use uuid::Uuid;

use std::time::Duration;

use be_block::BlockContent;
use be_block::database_view::{DatabaseView, DatabaseViewContent};

fn seed(instances: &mut Instances, block: Uuid, database: Uuid) {
    instances.editor_message(EditorMessage::SeedContent {
        instance: INSTANCE,
        block_id: block.into_bytes(),
        content_type: DatabaseViewContent::CONTENT_TYPE.into_bytes(),
        bytes: DatabaseViewContent::new(&DatabaseView::of(database)).encode(),
    });
}

fn database_in(bytes: &[u8]) -> Option<Uuid> {
    DatabaseViewContent::decode(bytes).ok()?.root().database
}

#[test]
fn a_database_view_given_content_references_its_database() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let view = Uuid::new_v4();
    let (database, other) = (Uuid::new_v4(), Uuid::new_v4());
    let mut instances = placed_on(view, DatabaseViewContent::CONTENT_TYPE);

    seed(&mut instances, view, database);
    instances.next_screens(PASS);
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        let held = shared.blocks.get(&view)?;
        (database_in(&held.bytes) == Some(database)).then_some(())
    })
    .expect("the view never took the content it was given");

    crate::be::flush();
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        let node = shared.graph.get(view)?;
        (node.references == [database]).then_some(())
    })
    .expect("the graph never learned what the view references");

    seed(&mut instances, view, other);
    crate::be::flush();
    let held = crate::be::content(view).expect("the view is still open");
    assert_eq!(database_in(&held.bytes), Some(database));
}
