use std::collections::BTreeMap;
use std::time::Instant;

use be_block_content::LiveEdit;
use be_sdk_type_guest::editor::{FromEditor, ToEditor};
use be_sdk_type_testing::sections::SetSection;

use super::*;

/// An operation can name a part by its ref, as one a peer put before it; the replica's module
/// then reads it with `read-part` for an editor. A part of this block reads, while a part of a
/// block the replica was never handed is refused with `OUTSIDE_REACH`, though this device
/// holds its key: a peer who reads both cannot show one block's bytes as the other's.
#[test]
fn an_editor_reads_no_part_of_another_block_its_replica_was_not_handed() {
    with_test_types(|peer, shared, mut replicas| async move {
        let [mine, other] = [block(&peer).await, block(&peer).await];
        let foreign = peer
            .put_part(other, b"another block's")
            .await
            .expect("the part is put");
        let own = peer
            .put_part(mine, b"this block's")
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
        for (index, part) in [(1, foreign), (2, own)] {
            let named = SetSection {
                index,
                bytes: Vec::new(),
                part: Some(part),
            };
            replicas.relay_from_editor(
                mine,
                editor,
                FromEditor::Operate(Sections::encode_operation(&named)).encode(),
                Instant::now(),
            );
        }
        replicas.pump((&peer, &shared)).await;

        // Every other section is empty, so section 1 starts right after the header.
        let header = (sections::SECTIONS * 4) as u64;
        for (token, offset, part) in [(1, header, foreign), (2, header + foreign.length(), own)] {
            replicas.relay_from_editor(
                mine,
                editor,
                FromEditor::Read {
                    token,
                    offset,
                    length: part.length(),
                }
                .encode(),
                Instant::now(),
            );
        }
        let mut ranges = BTreeMap::new();
        while ranges.len() < 2 {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
            for (to, frame) in std::mem::take(&mut shared.lock().unwrap().frames) {
                let message = frame
                    .split_first_chunk::<4>()
                    .and_then(|(_, payload)| ToEditor::decode(payload))
                    .or_else(|| ToEditor::decode(&frame));
                if to == editor
                    && let Some(ToEditor::Range { token, result }) = message
                {
                    ranges.insert(token, result);
                }
            }
        }
        assert_eq!(
            ranges[&2],
            Ok(b"this block's".to_vec()),
            "a part of its block reads"
        );
        assert!(
            matches!(&ranges[&1], Err(error) if error.contains(crate::replicas::OUTSIDE_REACH)),
            "a part of another block is refused: {:?}",
            ranges[&1]
        );
    });
}
