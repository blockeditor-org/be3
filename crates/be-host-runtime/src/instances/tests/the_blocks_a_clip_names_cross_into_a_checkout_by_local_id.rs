use super::*;
use uuid::Uuid;

use be_block::version_control::{masked, scope_mask};
use be_block::{BlockContent, BlockMetadata, CheckoutContent};
use be_block_graph::BlockParent;
use be_sdk_api::ClipItem;
use be_sdk_api::clipboard::{ClipBody, formats};
use be_type_text::TextContent;

use crate::calls::{AppCall, EditorAnswer, Outbound};

#[test]
fn the_blocks_a_clip_names_cross_into_a_checkout_by_local_id() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (checkout, local, elsewhere) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let copy = masked(local, scope_mask(checkout));
    be_host_stack::create(
        checkout,
        CheckoutContent::CONTENT_TYPE,
        BlockParent::Root,
        BlockMetadata::default(),
        None,
        false,
    );
    be_host_stack::create(
        copy,
        TextContent::CONTENT_TYPE,
        BlockParent::Block(checkout),
        {
            let mut metadata = BlockMetadata::default();
            metadata.insert(&be_block::LOCAL_ID, &local);
            metadata
        },
        None,
        false,
    );
    be_host_stack::create(
        elsewhere,
        TextContent::CONTENT_TYPE,
        BlockParent::Root,
        BlockMetadata::default(),
        None,
        false,
    );
    let instances = placed_on(copy, TextContent::CONTENT_TYPE);
    let screen = instances
        .screen_id(INSTANCE, REGION)
        .expect("the instance has a screen");

    let pasted = |refs: Vec<Uuid>| {
        let mut item = ClipItem::new("be3/any-format", vec![1, 2, 3]);
        item.refs = refs;
        Outbound::App(AppCall::Input(be_sdk_api::InputBatch {
            screen,
            events: vec![be_sdk_api::TimedInput {
                input: beui::Input::Paste(item),
                time_micros: None,
                over: None,
            }],
            every_sample: false,
            dismissed: Vec::new(),
        }))
    };
    let mut paste = pasted(vec![copy, elsewhere]);
    instances.translate_pastes(&mut paste);
    assert_eq!(
        paste,
        pasted(vec![local, elsewhere]),
        "a paste names the blocks by the ids the editor speaks in, whatever its format"
    );

    let rendered = |refs: Vec<Uuid>| {
        (
            INSTANCE,
            EditorAnswer::Rendered {
                request_id: 1,
                body: Some(ClipBody {
                    data: Vec::new(),
                    refs,
                }),
            },
        )
    };
    let mut answer = rendered(vec![local, elsewhere]);
    instances.translate(&mut answer, true);
    assert_eq!(
        answer,
        rendered(vec![copy, elsewhere]),
        "a representation rendered on request names the real blocks"
    );

    let dragged = |refs: Vec<Uuid>| {
        let mut item = ClipItem::new(formats::CANVAS_ITEMS, Vec::new());
        item.refs = refs;
        Outbound::Event(
            INSTANCE,
            be_sdk_api::app::EditorEvent::Drag(Some(be_sdk_api::shell_types::DragOver {
                view: REGION,
                pos: Pos2::ZERO,
                items: vec![item],
                action: be_sdk_api::DropAction::Copy,
                dropped: true,
            })),
        )
    };
    let mut drop = dragged(vec![copy]);
    instances.translate(&mut drop, false);
    assert_eq!(drop, dragged(vec![local]));
}
