use super::*;
use uuid::Uuid;

use be_block::version_control::{masked, scope_mask};
use be_block::{BlockContent, BlockMetadata, CheckoutContent, TextContent};
use be_graph::BlockParent;
use block_plugin_api::{BlockInfo, BlockLocation, BlockQuery};

fn info(block: Uuid, parent: Uuid, references: Vec<Uuid>) -> BlockInfo {
    BlockInfo {
        block_id: block.into_bytes(),
        block_type: TextContent::CONTENT_TYPE.into_bytes(),
        author: [0; 16],
        parent: BlockLocation::Block(parent.into_bytes()),
        name: None,
        named_by_hand: false,
        references: references.into_iter().map(Uuid::into_bytes).collect(),
        access: block_plugin_api::AccessLevel::Edit,
        artifact: None,
    }
}

#[test]
fn an_instance_inside_a_checkout_speaks_in_its_local_ids() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let (checkout, local, elsewhere) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let copy = masked(local, scope_mask(checkout));
    crate::be::create(
        checkout,
        CheckoutContent::CONTENT_TYPE,
        BlockParent::Root,
        BlockMetadata::default(),
        None,
    );
    crate::be::create(
        copy,
        TextContent::CONTENT_TYPE,
        BlockParent::Block(checkout),
        BlockMetadata {
            local_id: Some(local),
            ..BlockMetadata::default()
        },
        None,
    );
    crate::be::create(
        elsewhere,
        TextContent::CONTENT_TYPE,
        BlockParent::Root,
        BlockMetadata::default(),
        None,
    );
    let instances = placed_on(copy, TextContent::CONTENT_TYPE);

    let mut answer = Message::Editor(EditorMessage::Blocks {
        instance: INSTANCE,
        query: BlockQuery::Block(copy.into_bytes()),
        blocks: vec![info(copy, checkout, vec![copy, elsewhere])],
    });
    instances.translate(&mut answer, false);
    assert_eq!(
        answer,
        Message::Editor(EditorMessage::Blocks {
            instance: INSTANCE,
            query: BlockQuery::Block(local.into_bytes()),
            blocks: vec![info(local, checkout, vec![local, elsewhere])],
        })
    );

    let fresh = Uuid::new_v4();
    let mut created = Message::Editor(EditorMessage::CreateBlock {
        instance: INSTANCE,
        block_id: fresh.into_bytes(),
        content_type: TextContent::CONTENT_TYPE.into_bytes(),
        parent: BlockLocation::Block(local.into_bytes()),
        name: None,
        artifact: None,
        content: None,
    });
    instances.translate(&mut created, true);
    assert_eq!(
        created,
        Message::Editor(EditorMessage::CreateBlock {
            instance: INSTANCE,
            block_id: fresh.into_bytes(),
            content_type: TextContent::CONTENT_TYPE.into_bytes(),
            parent: BlockLocation::Block(copy.into_bytes()),
            name: None,
            artifact: None,
            content: None,
        })
    );

    let outside = placed_on(elsewhere, TextContent::CONTENT_TYPE);
    let mut unscoped = Message::Editor(EditorMessage::SetParent {
        instance: INSTANCE,
        block_id: copy.into_bytes(),
        parent: BlockLocation::Block(checkout.into_bytes()),
    });
    let before = unscoped.clone();
    outside.translate(&mut unscoped, true);
    assert_eq!(
        unscoped, before,
        "an editor outside every checkout sees real ids"
    );
}
