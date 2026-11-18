use be_block_content::BlockContent;
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

/// The laptop's edit is in the server's order. Offline, this device numbers one of its own and
/// stops before anything commits it: its journal is the edit's only copy. Started again online,
/// the replica opens on that journal rather than join at once, which would replace it with the
/// server's; it seals the edit, and joins once a drain sent the seal, holding both.
#[test]
fn an_edit_numbered_offline_before_a_restart_survives_the_first_join_online() {
    with_another_device(|peer, laptop, shared, replicas| async move {
        let block = peer
            .create::<TextContent>(BlockParent::Root)
            .await
            .expect("the text is made");
        peer.save(block, &TextBlock::of(""), None)
            .await
            .expect("the empty text is published");

        let laptop = Arc::new(laptop);
        let laptop_shared = Arc::new(Mutex::new(Quiet::default()));
        let mut laptops = Replicas::new(test_modules(be_block_client::Clock::real()).await);
        laptops
            .open(
                (&laptop, &laptop_shared),
                block,
                TextContent::CONTENT_TYPE,
                true,
            )
            .await
            .expect("the laptop opens the text");
        while laptops.joining() {
            laptops.woken().await;
            laptops.pump((&laptop, &laptop_shared)).await;
        }
        let mut mirror =
            attach_mirror::<TextContent>(&laptop, &laptop_shared, &mut laptops, block, 7).await;
        type_at_start(&mut laptops, block, 7, &mut mirror, "socks");
        laptops.pump((&laptop, &laptop_shared)).await;
        while laptops
            .submissions_of(block)
            .is_none_or(|(submitting, _, kept)| submitting > 0 || kept.is_empty())
        {
            laptops.woken().await;
            laptops.pump((&laptop, &laptop_shared)).await;
        }
        // The laptop goes away before sealing: its edit stays in the server's journal alone, and
        // its lease with it.
        drop(laptops);
        laptop.connection().close();
        laptop.detach();

        let (connection, token) = (peer.connection(), peer.token());
        peer.detach();
        let mut replicas = replicas;
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await
            .expect("the text opens offline");
        replicas.pump((&peer, &shared)).await;
        let mut mirror =
            attach_mirror::<TextContent>(&peer, &shared, &mut replicas, block, 8).await;
        type_at_start(&mut replicas, block, 8, &mut mirror, "boots ");
        replicas.pump((&peer, &shared)).await;
        replicas.modules.tick_due();
        replicas.pump((&peer, &shared)).await;
        let offline = peer
            .local_journal(block)
            .expect("the journal reads")
            .is_some_and(|journal| journal.entries.iter().any(|entry| entry.received.is_none()));
        assert!(offline, "the edit is numbered on this device alone");
        drop(replicas);

        peer.attach(connection, token)
            .await
            .expect("the device is connected again");
        let mut replicas = Replicas::new(test_modules(be_block_client::Clock::real()).await);
        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await
            .expect("the text opens online");
        replicas.pump((&peer, &shared)).await;
        assert!(
            !replicas.joining(),
            "it opens on this device's journal, not joining at once"
        );
        // As the worker does: the seal goes in the outbox, a drain sends it, and the replica
        // joins.
        replicas.keep_all((&peer, &shared)).await;
        replicas.pump((&peer, &shared)).await;
        peer.drain(|_| false).await.expect("the seal is sent");
        replicas.created((&peer, &shared));
        while replicas.joining() {
            step(&peer, &shared, &mut replicas).await;
        }
        // Opened on the server's journal, it merges in the seal's head once told of it.
        while replicas.status(block).is_none_or(|status| {
            !status.clean || status.head != peer.local_head(block)
        }) {
            step(&peer, &shared, &mut replicas).await;
        }
        let text = text(&peer, &shared, &mut replicas, block).await;
        assert!(
            text.contains("boots") && text.contains("socks"),
            "the edit numbered offline survives the first join beside the laptop's: {text:?}"
        );
    });
}
