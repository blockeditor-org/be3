use std::time::Instant;

use be_block_content::{BlockContent, LiveEdit};
use be_sdk_type_api::Status;
use be_sdk_type_guest::editor::FromEditor;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// Types `typed` at the start of the open text as its editor.
async fn type_in(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
    block: Uuid,
    editor: u64,
    typed: &[u8],
) {
    let content = TextContent::decode(
        &replicas
            .content((peer, shared), block)
            .await
            .expect("the text is open"),
    )
    .expect("the text decodes");
    let operation = TextBlock::insert(&content, 1, 0, typed).expect("there is somewhere to type");
    replicas.relay_from_editor(
        block,
        editor,
        FromEditor::Operate(TextContent::encode_operation(&operation)).encode(),
        Instant::now(),
    );
    replicas.pump((peer, shared)).await;
}

/// An open text takes an edit, and then a status its module made before it took that edit
/// arrives: clean, and counting no edit taken. The host keeps the edit all the same, so the
/// replica that opens again on joining its session (here as the host reconnects; My files'
/// first join did so) is given it rather than losing it.
#[test]
fn an_edit_sent_after_its_replica_made_a_clean_status_is_given_again_on_joining() {
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
        replicas.attach(block, editor, true);
        replicas.pump((&peer, &shared)).await;
        type_in(&peer, &shared, &mut replicas, block, editor, b"wool ").await;
        replicas.seal((&peer, &shared), block, true).await;
        while replicas
            .status(block)
            .is_none_or(|status| !status.clean || status.head.is_none())
        {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let clean = replicas.status(block).expect("the text is open");
        type_in(&peer, &shared, &mut replicas, block, editor, b"socks ").await;
        // The status the module made before it took the edit reaches the host only now.
        replicas
            .report((&peer, &shared), block, Status { taken: 0, ..clean })
            .await;

        replicas.reconnect((&peer, &shared));
        assert!(replicas.joining(), "it joins its session again");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        replicas.pump((&peer, &shared)).await;
        let bytes = replicas
            .content((&peer, &shared), block)
            .await
            .expect("the text is open");
        let text = TextBlock::text(&TextContent::decode(&bytes).expect("the text decodes"));
        assert_eq!(
            text, "socks wool ",
            "the edit survives the replica opening again"
        );
    });
}
