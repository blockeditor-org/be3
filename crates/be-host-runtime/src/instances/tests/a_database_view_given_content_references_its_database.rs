use super::*;
use uuid::Uuid;

use be_block::BlockContent;
use be_type_database::database_view::{DatabaseView, DatabaseViewContent};

fn seed(instances: &mut Instances, block: Uuid, database: Uuid) {
    call(
        instances,
        Call::WriteContent {
            reply: reply(next_op()),
            block: block.into_bytes(),
            content_type: DatabaseViewContent::CONTENT_TYPE.into_bytes(),
            bytes: DatabaseViewContent::new(&DatabaseView::of(database)).encode(),
            replace: false,
        },
    );
}

fn database_in(bytes: &[u8]) -> Option<Uuid> {
    DatabaseViewContent::decode(bytes).ok()?.root().database
}

#[test]
fn a_database_view_given_content_references_its_database() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let view = Uuid::new_v4();
    let (database, other) = (Uuid::new_v4(), Uuid::new_v4());
    let mut instances = placed_on(view, DatabaseViewContent::CONTENT_TYPE);
    be_host_stack::wait_until(|_| {
        be_host_stack::is_known(DatabaseViewContent::CONTENT_TYPE).then_some(())
    });

    seed(&mut instances, view, database);
    be_host_stack::flush();
    to_replica(
        &mut instances,
        view,
        &be_sdk_type_guest::editor::FromEditor::Resend,
    );
    instances.next_screens(PASS);
    be_host_stack::wait_until(|shared| {
        let held = shared.blocks.get(&view)?;
        (database_in(held.bytes()) == Some(database)).then_some(())
    });

    be_host_stack::flush();
    be_host_stack::wait_until(|shared| {
        let node = shared.graph.get(view)?;
        (node.references == [database]).then_some(())
    });

    seed(&mut instances, view, other);
    be_host_stack::flush();
    let held = be_host_stack::content(view).expect("the view is still open");
    assert_eq!(database_in(held.bytes()), Some(database));
}
