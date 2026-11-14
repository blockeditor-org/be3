use std::time::Instant;

use be_block_content::BlockContent;
use be_sdk_type_guest::editor::{EditorMirror, FromEditor, ToEditor};
use be_type_text::{TextBlock, TextContent};

use super::*;

/// What the open text holds now, as its replica shows it.
async fn text(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
    block: Uuid,
) -> String {
    let bytes = replicas
        .content((peer, shared), block)
        .await
        .expect("the text is open");
    TextBlock::text(&TextContent::decode(&bytes).expect("the text decodes"))
}

/// Gives `mirror` what the replica sent `editor` since it was last given.
fn receive(shared: &Arc<Mutex<Quiet>>, editor: EditorId, mirror: &mut EditorMirror<TextContent>) {
    let frames: Vec<Vec<u8>> = std::mem::take(&mut shared.lock().unwrap().frames)
        .into_iter()
        .filter(|(to, _)| *to == editor)
        .map(|(_, frame)| frame)
        .collect();
    for frame in frames {
        let message = frame
            .split_first_chunk::<4>()
            .filter(|(length, payload)| u32::from_be_bytes(**length) as usize == payload.len())
            .and_then(|(_, payload)| ToEditor::decode(payload))
            .or_else(|| ToEditor::decode(&frame));
        if let Some(message) = message {
            mirror.receive(message);
        }
    }
}

/// Types `text` at the start of what `mirror` shows, as an editor does.
fn type_at_start(
    replicas: &mut Replicas<Quiet>,
    block: Uuid,
    editor: EditorId,
    mirror: &mut EditorMirror<TextContent>,
    text: &str,
) {
    let shown = mirror.content().expect("the editor was sent the text");
    let typed =
        TextBlock::insert(shown, 3, 0, text.as_bytes()).expect("there is somewhere to type");
    let message = mirror.operate(typed).expect("the editor has content");
    replicas.relay_from_editor(block, editor, message.encode(), Instant::now());
}

/// A replica that joins its session reopens on its head, and an editor's next edit is numbered
/// by the content it was shown before: the session's state as of that head (where each writer's
/// next edit starts) comes from the journal on it, this device's when the server's is on
/// another commit, so the edit lands rather than being taken for one out of order.
#[test]
fn an_edit_made_on_the_content_from_before_a_replica_joins_lands_after() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = Uuid::new_v4();
        peer.keep_creation(
            block,
            TextContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::default(),
            false,
        )
        .await
        .expect("the block is kept here");
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await;
        replicas.pump((&peer, &shared)).await;
        let editor = 7;
        let mut mirror = EditorMirror::<TextContent>::default();
        replicas.attach(block, editor, true);
        replicas.relay_from_editor(block, editor, FromEditor::Resend.encode(), Instant::now());
        replicas.pump((&peer, &shared)).await;
        receive(&shared, editor, &mut mirror);
        while mirror.content().is_none() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
            receive(&shared, editor, &mut mirror);
        }
        type_at_start(&mut replicas, block, editor, &mut mirror, "socks");
        replicas.pump((&peer, &shared)).await;
        replicas.keep_all((&peer, &shared)).await;
        replicas.pump((&peer, &shared)).await;
        // What the editor is shown before the replica joins.
        receive(&shared, editor, &mut mirror);
        assert_eq!(TextBlock::text(mirror.content().expect("shown")), "socks");

        peer.drain(|_| false).await.expect("the creation is sent");
        replicas.created((&peer, &shared));
        assert!(replicas.joining(), "once sent, the block joins its session");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        replicas.pump((&peer, &shared)).await;

        type_at_start(&mut replicas, block, editor, &mut mirror, "boots ");
        replicas.pump((&peer, &shared)).await;
        assert_eq!(
            text(&peer, &shared, &mut replicas, block).await,
            "boots socks",
            "the edit made on the content shown before the replica joined lands"
        );
    });
}
