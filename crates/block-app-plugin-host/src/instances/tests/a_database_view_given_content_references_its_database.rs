use super::*;
use uuid::Uuid;

use std::time::Duration;

use be_block::BlockContent;
use database_types::database_view::{DatabaseView, DatabaseViewContent};

fn seed(instances: &mut Instances, block: Uuid, database: Uuid) {
    call(
        instances,
        Call::SeedContent {
            block: block.into_bytes(),
            content_type: DatabaseViewContent::CONTENT_TYPE.into_bytes(),
            bytes: DatabaseViewContent::new(&DatabaseView::of(database)).encode(),
        },
    );
}

fn database_in(bytes: &[u8]) -> Option<Uuid> {
    DatabaseViewContent::decode(bytes).ok()?.root().database
}

#[test]
fn a_database_view_given_content_references_its_database() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    let view = Uuid::new_v4();
    let (database, other) = (Uuid::new_v4(), Uuid::new_v4());
    let mut instances = placed_on(view, DatabaseViewContent::CONTENT_TYPE);

    seed(&mut instances, view, database);
    block_app_stack::flush();
    to_replica(
        &mut instances,
        view,
        &block_type_guest::editor::FromEditor::Resend,
    );
    instances.next_screens(PASS);
    block_app_stack::wait_for(Duration::from_secs(20), |shared| {
        let held = shared.blocks.get(&view)?;
        (database_in(held.bytes()) == Some(database)).then_some(())
    })
    .expect("the view never took the content it was given");

    block_app_stack::flush();
    block_app_stack::wait_for(Duration::from_secs(20), |shared| {
        let node = shared.graph.get(view)?;
        (node.references == [database]).then_some(())
    })
    .expect("the graph never learned what the view references");

    seed(&mut instances, view, other);
    block_app_stack::flush();
    let held = block_app_stack::content(view).expect("the view is still open");
    assert_eq!(database_in(held.bytes()), Some(database));
}
