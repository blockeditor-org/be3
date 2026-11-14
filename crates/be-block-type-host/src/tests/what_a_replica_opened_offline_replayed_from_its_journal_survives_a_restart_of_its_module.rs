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
/// which no editor gives it again. Its module stops, unplanned, before anything seals that: the
/// replica opened again on its head with this device's whole journal there replays it again.
#[test]
fn what_a_replica_opened_offline_replayed_from_its_journal_survives_a_restart_of_its_module() {
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

        replicas.modules.restart_type(TextContent::CONTENT_TYPE);
        replicas.pump((&peer, &shared)).await;
        assert_eq!(
            text(&peer, &shared, &mut replicas, block).await,
            "socks",
            "what the replica replayed from its journal survives a restart of its module"
        );
    });
}
