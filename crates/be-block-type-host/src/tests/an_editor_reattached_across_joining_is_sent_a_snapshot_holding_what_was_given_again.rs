use be_block_content::{BlockContent, LiveEdit};
use be_sdk_type_api::Status;
use be_sdk_type_guest::editor::ToEditor;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// What `editor` was sent since the frames were last taken.
fn sent_to(shared: &Arc<Mutex<Quiet>>, editor: u64) -> Vec<ToEditor> {
    let frames = std::mem::take(&mut shared.lock().unwrap().frames);
    frames
        .into_iter()
        .filter(|(to, _)| *to == editor)
        .filter_map(|(_, frame)| {
            frame
                .split_first_chunk::<4>()
                .filter(|(length, payload)| u32::from_be_bytes(**length) as usize == payload.len())
                .and_then(|(_, payload)| ToEditor::decode(payload))
                .or_else(|| ToEditor::decode(&frame))
        })
        .collect()
}

/// A replica opened again as its session is joined holds the sealed edit of an editor still
/// open, and is given again the one it took after: the editor is sent one snapshot, after
/// both, counting both, so it lets go of neither and rebuilds nothing on top of what is
/// missing. Its next edit lands as its own.
#[test]
fn an_editor_reattached_across_joining_is_sent_a_snapshot_holding_what_was_given_again() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = peer
            .create::<TextContent>(BlockParent::Root)
            .await
            .expect("the text is made");
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await;
        replicas.pump((&peer, &shared)).await;
        let editor = 7;
        let mut mirror =
            attach_mirror::<TextContent>(&peer, &shared, &mut replicas, block, editor).await;
        type_at_start(&mut replicas, block, editor, &mut mirror, "wool ");
        replicas.pump((&peer, &shared)).await;
        replicas.seal((&peer, &shared), block, true).await;
        while replicas
            .status(block)
            .is_none_or(|status| !status.clean || status.head.is_none())
        {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let clean = replicas.status(block).expect("the text is open");
        type_at_start(&mut replicas, block, editor, &mut mirror, "socks ");
        replicas.pump((&peer, &shared)).await;
        // The status the module made before it took the edit reaches the host only now.
        replicas
            .report((&peer, &shared), block, Status { taken: 0, ..clean })
            .await;
        for message in sent_to(&shared, editor) {
            mirror.receive(message);
        }

        replicas.reconnect((&peer, &shared));
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        // Asked after the replica's `resent`, so answered once the editor was sent its snapshot.
        let bytes = replicas
            .content((&peer, &shared), block)
            .await
            .expect("the text is open");
        let text = TextBlock::text(&TextContent::decode(&bytes).expect("the text decodes"));
        assert_eq!(text, "socks wool ", "the edit given again is in");
        replicas.pump((&peer, &shared)).await;
        let received = sent_to(&shared, editor);
        let snapshots: Vec<_> = received
            .iter()
            .filter_map(|message| match message {
                ToEditor::Snapshot(snapshot) => Some(snapshot),
                _ => None,
            })
            .collect();
        let [snapshot] = snapshots.as_slice() else {
            panic!("the editor is sent one snapshot, not {}", snapshots.len());
        };
        assert_eq!(
            snapshot.applied, 2,
            "it counts the sealed edit and the one given again"
        );
        let mut shown = TextContent::decode(&snapshot.bytes).expect("the snapshot decodes");
        for operation in &snapshot.unsettled {
            shown.apply(&TextContent::decode_operation(operation).expect("an operation"));
        }
        assert_eq!(TextBlock::text(&shown), "socks wool ");
        for message in received {
            mirror.receive(message);
        }
        assert_eq!(
            mirror.pending(),
            0,
            "the editor let go of both of its edits"
        );

        type_at_start(&mut replicas, block, editor, &mut mirror, "red ");
        replicas.pump((&peer, &shared)).await;
        let mine: Vec<bool> = sent_to(&shared, editor)
            .into_iter()
            .flat_map(|message| match message {
                ToEditor::Operations(operations) => operations,
                _ => Vec::new(),
            })
            .map(|operation| operation.mine)
            .collect();
        assert_eq!(mine, [true], "the next edit lands as the editor's own");
        let bytes = replicas
            .content((&peer, &shared), block)
            .await
            .expect("the text is open");
        let text = TextBlock::text(&TextContent::decode(&bytes).expect("the text decodes"));
        assert_eq!(text, "red socks wool ");
    });
}
