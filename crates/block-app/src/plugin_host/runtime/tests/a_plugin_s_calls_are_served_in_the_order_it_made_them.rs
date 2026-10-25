use super::*;
use crate::plugin_host::EditorBlock;
use block_plugin_api::{
    BlockLocation, MetadataEntries, OpenBlock,
    __args::{
        BLOCKS_CREATE_BLOCK_NAME, BlocksCreateBlockArgs, HOST_SERVICES_READ_PLAIN_FILE,
        HostServicesReadPlainFileArgs,
    },
    ops::{Connection, Incoming, OpId, Outcome, Side, decode, encode},
};
use be_block::BlockContent as _;
use text_block_types::TextContent;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);

fn carry(from: Vec<Vec<u8>>, to: &mut Connection) -> Vec<Incoming> {
    for frame in from {
        to.receive(&frame).expect("the frame decodes");
    }
    std::iter::from_fn(|| to.next_event()).collect()
}

fn opened(runtime: &mut Runtime, guest: &mut Connection) -> OpId {
    let size = vec2(100.0, 100.0);
    runtime.instances.report(
        INSTANCE,
        EditorRegion::Frame,
        Uuid::nil(),
        InstanceRole::Editor(
            Uuid::nil(),
            EditorBlock {
                id: Uuid::nil(),
                block_type: Uuid::nil(),
                view_block: None,
            },
        ),
        &Arc::new(block_plugin_api::Catalog::default()),
        Some(FrameSpec::default()),
        size,
        Rect::from_min_size(Pos2::ZERO, size),
        1.0,
        1,
    );
    let wire = runtime.backend.wire_mut();
    wire.open(Opening::Block(OpenBlock {
        instance: INSTANCE,
        editor: [0; 16],
        block_id: [0; 16],
        block_type: [0; 16],
        view_block: None,
        account_id: [4; 16],
        organization_id: [5; 16],
        client_id: [6; 16],
        editable: true,
    }));
    match carry(wire.take_outbound(), guest).as_slice() {
        [Incoming::Started(started)] => started.op,
        heard => panic!("the plugin heard {heard:?} rather than its editor opening"),
    }
}

fn refused(outcome: &Outcome) -> bool {
    let read: Option<Result<block_plugin_api::SavedFile, String>> = match outcome {
        Outcome::Ok(bytes) => decode(bytes),
        Outcome::Err(_) => None,
    };
    matches!(read, Some(Err(error)) if error.contains("can't view"))
}

#[test]
fn a_plugin_s_calls_are_served_in_the_order_it_made_them() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let plugin = block_plugin_api::manifest_from_json(
        r#"{
            "id": "test.ordered",
            "name": "Ordered",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e418",
                "edits": "00000000-0000-4000-8000-0000000000c4",
                "display_name": "Ordered",
                "icon": "x",
                "regions": ["Frame"]
            }},
            "entry_point": "test.ordered.wasm"
        }"#,
    )
    .expect("the manifest is valid");
    let mut runtime = Runtime::new(&plugin, 0);
    let mut guest = Connection::new(Side::Guest);
    let editor = opened(&mut runtime, &mut guest);

    let fresh = Uuid::new_v4();
    let create = BlocksCreateBlockArgs {
        block: fresh.into_bytes(),
        block_type: TextContent::CONTENT_TYPE.into_bytes(),
        parent: BlockLocation::Root,
        defined: MetadataEntries::new(),
        content: None,
    };
    guest.notify_direct(Some(editor), BLOCKS_CREATE_BLOCK_NAME, &encode(&create));
    let read = guest.start(
        Some(editor),
        HOST_SERVICES_READ_PLAIN_FILE,
        encode(&HostServicesReadPlainFileArgs {
            block: fresh.into_bytes(),
        }),
    );
    let mut heard = Vec::new();
    for frame in guest.take_outbound() {
        runtime
            .backend
            .wire_mut()
            .receive(&frame, &mut heard)
            .expect("the plugin's frames decode");
    }

    runtime.taking_in = false;
    runtime.hear(heard);
    assert!(
        crate::be::node(fresh).is_none() && !crate::be::access(fresh).can_view(),
        "calls heard while a frame is drawn wait for the next one, and a read served now would be refused"
    );
    let answered = carry(runtime.backend.wire_mut().take_outbound(), &mut guest);
    assert!(
        answered.is_empty(),
        "the read waits behind the create it follows: {answered:?}"
    );

    runtime.taking_in = true;
    runtime.hear(Vec::new());
    assert!(
        crate::be::access(fresh).can_view(),
        "the block was created first"
    );
    let answered = carry(runtime.backend.wire_mut().take_outbound(), &mut guest);
    assert!(
        !answered.iter().any(|event| matches!(
            event,
            Incoming::Completed { op, outcome } if *op == read && refused(outcome)
        )),
        "the read is served after the create, so the block is there to read: {answered:?}"
    );
}
