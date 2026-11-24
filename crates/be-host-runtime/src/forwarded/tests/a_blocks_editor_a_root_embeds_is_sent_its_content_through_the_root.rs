use std::{cell::Cell, cell::RefCell, rc::Rc};

use be_sdk_api::{
    guest::blocks::Replica,
    ops::guest::{Executor, Serve, spawn, with_parent},
};
use be_sdk_type_guest::editor::{FromEditor, ToEditor};
use be_block_content::BlockContent as _;
use be_type_example_counter::CounterContent;

use super::*;
use crate::EditorBlock;

const INSTANCE: EditorInstanceId = EditorInstanceId(11);

fn manifest() -> AppManifest {
    be_sdk_manifest::manifest_from_json(
        r#"{
            "id": "test.counter",
            "name": "Counter",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000c0de",
                "edits": "00000000-0000-4000-8000-0000000000d4",
                "display_name": "Counter",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.counter.wasm"
        }"#,
    )
    .expect("the manifest is valid")
}

/// Passes frames between the app, the root's wire and the host's end until none moves.
fn exchange(root: &mut Wire, forwarded: &mut Forwarded, guest: &Executor, relayed: Vec<Vec<u8>>) {
    let mut relayed = relayed;
    loop {
        let moved = !relayed.is_empty();
        root.relay_forwarded(std::mem::take(&mut relayed))
            .expect("the host's frames decode");
        let mut items = Vec::new();
        guest
            .step(root.take_outbound(), &mut items)
            .expect("the root's frames decode");
        let mut heard = Vec::new();
        for item in &items {
            root.receive(item, &mut heard)
                .expect("the app's frames decode");
        }
        assert!(heard.is_empty(), "the root hears nothing of the editor");
        relayed = forwarded
            .receive(root.take_forwarded())
            .expect("the forwarded frames are the protocol's");
        if !moved && items.is_empty() && relayed.is_empty() {
            return;
        }
    }
}

#[test]
fn a_blocks_editor_a_root_embeds_is_sent_its_content_through_the_root() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    be_host_stack::create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );

    let editor = Rc::new(Cell::new(None));
    let opened = Rc::clone(&editor);
    let guest = Executor::new(Box::new(move |_, started| {
        // The editor, which the host opens under nothing; what it tells the editor after
        // (`editor.hear`) it starts under it.
        if started.parent.is_none() {
            opened.set(Some(started.op));
        }
        Serve::Resource {
            instance: Rc::new(()),
            on_drop: Box::new(|| Box::pin(async {})),
        }
    }));
    let mut root = Wire::default();
    let mut forwarded = Forwarded::new(&manifest());
    let opening = forwarded.open(
        INSTANCE,
        InstanceRole::Editor(
            Uuid::nil(),
            EditorBlock {
                id: block,
                block_type: CounterContent::CONTENT_TYPE,
                view_block: None,
            },
        ),
        Uuid::new_v4(),
        &Arc::new(crate::AppCatalog::default()),
    );
    exchange(&mut root, &mut forwarded, &guest, opening);
    let editor = editor
        .get()
        .expect("the app was sent the editor the host opened");

    let sent = Rc::new(RefCell::new(Vec::new()));
    let into = Rc::clone(&sent);
    guest.enter(|| {
        let replica = with_parent(editor, || {
            Replica::new(block.into_bytes(), CounterContent::CONTENT_TYPE.into_bytes())
        });
        let mut frames = replica.frames();
        let resend = replica.send(be_sdk_type_api::encode_frame(&FromEditor::Resend));
        spawn(async move {
            let _held = replica;
            let _ = resend.await;
            while let Some(frame) = frames.recv().await {
                into.borrow_mut().extend(
                    be_host_stack::unframe(&frame).and_then(|bytes| ToEditor::decode(&bytes)),
                );
            }
        });
    });
    let snapshot = || {
        sent.borrow()
            .iter()
            .any(|message| matches!(message, ToEditor::Snapshot(_)))
    };
    exchange(&mut root, &mut forwarded, &guest, Vec::new());
    while !snapshot() {
        be_host_stack::wait_for_frames("the editor's snapshot", INSTANCE.0);
        let relayed = forwarded.step();
        exchange(&mut root, &mut forwarded, &guest, relayed);
    }
}
