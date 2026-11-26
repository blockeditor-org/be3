use std::time::Instant;

use be_block_content::LiveEdit;
use be_sdk_type_guest::editor::{FromEditor, ToEditor};
use be_sdk_type_testing::sections::SetSection;

use super::*;

/// An editor's range read of a part its replica left in the store (a blob's payload, read once
/// the block opens again) waits on `read-part`, answered off the replicas' loop. Until the
/// editor has its bytes the replicas say they have yet to settle (`Replicas::unsettled`), so a
/// driver that paints once everything settled does not paint the editor before it has them.
#[test]
fn a_part_read_for_an_editor_keeps_the_replicas_unsettled_until_answered() {
    with_test_types(|peer, shared, mut replicas| async move {
        let mine = block(&peer).await;
        let own = put_part(&peer, mine, b"this block's")
            .await
            .expect("the part is put");
        replicas
            .open((&peer, &shared), mine, Sections::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let editor = 7;
        replicas.attach(mine, editor, true);
        replicas.pump((&peer, &shared)).await;
        let named = SetSection {
            index: 1,
            bytes: Vec::new(),
            part: Some(own),
        };
        replicas.relay_from_editor(
            mine,
            editor,
            FromEditor::Operate {
                gesture: None,
                operation: Sections::encode_operation(&named),
            }
            .encode(),
            Instant::now(),
        );
        replicas.pump((&peer, &shared)).await;
        shared.lock().unwrap().frames.clear();

        let header = (sections::SECTIONS * 4) as u64;
        replicas.relay_from_editor(
            mine,
            editor,
            FromEditor::Read {
                token: 1,
                offset: header,
                length: own.length(),
            }
            .encode(),
            Instant::now(),
        );
        let mut read = None;
        let mut reading = false;
        while read.is_none() {
            replicas.pump((&peer, &shared)).await;
            for (to, frame) in std::mem::take(&mut shared.lock().unwrap().frames) {
                let message = frame
                    .split_first_chunk::<4>()
                    .and_then(|(_, payload)| ToEditor::decode(payload))
                    .or_else(|| ToEditor::decode(&frame));
                if to == editor
                    && let Some(ToEditor::Range { result, .. }) = message
                {
                    read = Some(result);
                }
            }
            if read.is_some() {
                break;
            }
            let unsettled = replicas.unsettled();
            assert!(
                !unsettled.is_empty(),
                "the editor waits for its range, but the replicas say they settled"
            );
            reading |= unsettled.iter().any(|why| why.contains("part read"));
            replicas.woken().await;
        }
        assert_eq!(read, Some(Ok(b"this block's".to_vec())));
        assert!(reading, "the part read was waited on as unsettled");
        replicas.pump((&peer, &shared)).await;
        assert_eq!(replicas.unsettled(), Vec::<String>::new());
    });
}
