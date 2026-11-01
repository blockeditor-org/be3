use super::*;
use uuid::Uuid;

use be_block::version_control::{masked, scope_mask};
use be_block::{BlockContent, BlockMetadata, CheckoutContent};
use be_graph::BlockParent;
use block_plugin_api::{BlockInfo, BlockLocation, BlockQuery};
use text_block_types::TextContent;

fn info(block: Uuid, parent: Uuid, references: Vec<Uuid>) -> BlockInfo {
    BlockInfo {
        block_id: block.into_bytes(),
        block_type: TextContent::CONTENT_TYPE.into_bytes(),
        author: Some([0; 16]),
        parent: Some(BlockLocation::Block(parent.into_bytes())),
        references: references.into_iter().map(Uuid::into_bytes).collect(),
        access: block_plugin_api::AccessLevel::Edit,
        metadata: block_plugin_api::BlockMetadata::default(),
        locked: false,
        deleted: false,
        sets_access: false,
    }
}

#[test]
fn an_instance_inside_a_checkout_speaks_in_its_local_ids() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    let (checkout, local, elsewhere) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let copy = masked(local, scope_mask(checkout));
    block_app_stack::create(
        checkout,
        CheckoutContent::CONTENT_TYPE,
        BlockParent::Root,
        BlockMetadata::default(),
        None,
    );
    block_app_stack::create(
        copy,
        TextContent::CONTENT_TYPE,
        BlockParent::Block(checkout),
        {
            let mut metadata = BlockMetadata::default();
            metadata.insert(&be_block::LOCAL_ID, &local);
            metadata
        },
        None,
    );
    block_app_stack::create(
        elsewhere,
        TextContent::CONTENT_TYPE,
        BlockParent::Root,
        BlockMetadata::default(),
        None,
    );
    let instances = placed_on(copy, TextContent::CONTENT_TYPE);
    let scope = instances.scope(INSTANCE);

    let mut query = BlockQuery::Block(copy.into_bytes());
    let mut answer = info(copy, checkout, vec![copy, elsewhere]);
    Instances::translated(scope, false, |visit| {
        query.visit_block_ids(visit);
        answer.visit_block_ids(visit);
    });
    assert_eq!(query, BlockQuery::Block(local.into_bytes()));
    assert_eq!(answer, info(local, checkout, vec![local, elsewhere]));

    let fresh = Uuid::new_v4();
    let create = |parent: Uuid| Call::CreateBlock {
        reply: reply(0),
        block: fresh.into_bytes(),
        block_type: TextContent::CONTENT_TYPE.into_bytes(),
        parent: BlockLocation::Block(parent.into_bytes()),
        defined: block_plugin_api::MetadataEntries::new(),
        content: None,
    };
    let mut created = create(local);
    Instances::translated(scope, true, |visit| created.visit_block_ids(visit));
    assert_eq!(format!("{created:?}"), format!("{:?}", create(copy)));

    let outside = placed_on(elsewhere, TextContent::CONTENT_TYPE);
    let set_parent = || Call::SetParent {
        reply: reply(0),
        block: copy.into_bytes(),
        parent: BlockLocation::Block(checkout.into_bytes()),
    };
    let mut unscoped = set_parent();
    Instances::translated(outside.scope(INSTANCE), true, |visit| {
        unscoped.visit_block_ids(visit);
    });
    assert_eq!(
        format!("{unscoped:?}"),
        format!("{:?}", set_parent()),
        "an editor outside every checkout sees real ids"
    );
}
