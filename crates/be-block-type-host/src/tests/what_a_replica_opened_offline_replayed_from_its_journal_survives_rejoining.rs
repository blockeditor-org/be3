use std::time::{Duration, Instant};

use be_block_content::{BlockContent, LiveEdit};
use be_sdk_type_guest::editor::FromEditor;
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

/// A replica opened offline after a crash holds what it replayed from this device's journal,
/// which no editor gives it again. Reconnecting has it seal that first, and the rejoin's answer,
/// however early it comes, waits until the replica reports the commit that seal made: reopened
/// on the head it opened on meanwhile, it would sit beside its own seal, holding what that seal
/// holds a second time.
#[test]
fn what_a_replica_opened_offline_replayed_from_its_journal_survives_rejoining() {
    let clock = be_block_client::Clock::manual();
    let modules_clock = clock.clone();
    with_test_types_on(clock.clone(), |peer, shared, replicas| async move {
        let block = peer
            .create::<TextContent>(BlockParent::Root)
            .await
            .expect("the text is made");
        peer.save(block, &TextBlock::of(""), None)
            .await
            .expect("the empty text is published");
        let (connection, token) = (peer.connection(), peer.token());
        peer.detach();

        // Offline, an editor types; the replica journals it on this device and the device stops
        // before anything commits it.
        let mut replicas = replicas;
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await;
        replicas.pump((&peer, &shared)).await;
        let editor = 7;
        replicas.attach(block, editor, true);
        replicas.pump((&peer, &shared)).await;
        let content = TextContent::decode(
            &replicas
                .content((&peer, &shared), block)
                .await
                .expect("the text is open"),
        )
        .expect("the text decodes");
        let typed =
            TextBlock::insert(&content, 1, 0, b"socks").expect("there is somewhere to type");
        replicas.relay_from_editor(
            block,
            editor,
            FromEditor::Operate(TextContent::encode_operation(&typed)).encode(),
            Instant::now(),
        );
        replicas.pump((&peer, &shared)).await;
        modules_clock.advance(Duration::from_millis(be_sdk_type_guest::APPEND_INTERVAL));
        replicas.modules.tick_due();
        replicas.pump((&peer, &shared)).await;
        let journaled = peer
            .local_journal(block)
            .expect("the journal reads")
            .is_some_and(|journal| !journal.entries.is_empty());
        assert!(journaled, "the edit is journaled on this device");
        drop(replicas);

        // Started again, still offline: the replica replays the journal.
        let mut replicas = Replicas::new(test_modules(modules_clock.clone()).await);
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await;
        replicas.pump((&peer, &shared)).await;
        assert_eq!(text(&peer, &shared, &mut replicas, block).await, "socks");

        peer.attach(connection, token)
            .await
            .expect("the device is connected again");
        replicas.reconnect((&peer, &shared));
        // The module starts the seal reconnecting asked of it, and the server answers the rejoin
        // before the module reports where the seal left it: the answer waits for that.
        replicas.pump((&peer, &shared)).await;
        replicas.rejoin_answered().await;
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        replicas.pump((&peer, &shared)).await;
        assert_eq!(
            text(&peer, &shared, &mut replicas, block).await,
            "socks",
            "what the replica replayed from its journal survives its rejoining"
        );
        assert_eq!(
            replicas.status(block).and_then(|status| status.head),
            peer.local_head(block),
            "it reopens on the commit its seal made, not beside it on the head it opened on"
        );
    });
}
