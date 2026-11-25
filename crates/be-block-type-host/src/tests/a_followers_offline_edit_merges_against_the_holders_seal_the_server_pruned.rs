use be_block_store::ObjectStore;
use be_type_text::{TextBlock, TextContent};

use super::*;

type Heard = tokio::sync::broadcast::Receiver<be_block_client::Event>;

/// Waits for the laptop's modules or its connection, and hands its replicas all of it: the
/// test's `step` hears this device's connection alone.
async fn step_laptop(
    laptop: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    laptops: &mut Replicas<Quiet>,
    heard: &mut Heard,
) {
    let event = tokio::select! {
        () = laptops.woken() => None,
        event = heard.recv() => event.ok(),
    };
    if let Some(event) = event {
        laptops.heard((laptop, shared), &event);
    }
    while let Ok(event) = heard.try_recv() {
        laptops.heard((laptop, shared), &event);
    }
    laptops.pump((laptop, shared)).await;
}

/// Seals what the laptop typed once its order holds it, and answers the seal's commit, past
/// `past`.
async fn sealed_past(
    laptop: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    laptops: &mut Replicas<Quiet>,
    heard: &mut Heard,
    block: Uuid,
    past: Option<CommitId>,
) -> CommitId {
    let waited = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        laptops.pump((laptop, shared)).await;
        while laptops
            .submissions_of(block)
            .is_none_or(|(submitting, _, kept)| submitting > 0 || kept.is_empty())
        {
            step_laptop(laptop, shared, laptops, heard).await;
        }
        laptops.seal((laptop, shared), block, false).await;
        loop {
            if let Some(head) = laptops
                .status(block)
                .filter(|status| status.clean)
                .and_then(|status| status.head)
                .filter(|head| Some(*head) != past)
                && laptop.remote_head(block).await.ok().flatten() == Some(head)
            {
                return head;
            }
            step_laptop(laptop, shared, laptops, heard).await;
        }
    })
    .await;
    waited.expect("the laptop seals what it typed")
}

/// This device follows the laptop, which holds the text's lease, and takes the laptop's seal
/// of three lines as its head from the session alone. Offline, it types a line above them on
/// that seal and seals what it typed; the laptop meanwhile deletes the last line and seals
/// again, and the server prunes
/// the seal both edits were made on. This device fetched that seal as it heard of it, so its
/// edit merges against it once back online: the deletion stands, rather than conflicting with
/// the edit as it would in a merge with no base.
#[test]
fn a_followers_offline_edit_merges_against_the_holders_seal_the_server_pruned() {
    with_another_device(|peer, laptop, shared, mut replicas| async move {
        let block = peer
            .create::<TextContent>(BlockParent::Root)
            .await
            .expect("the text is made");
        peer.save(block, &TextBlock::of(""), None)
            .await
            .expect("the empty text is published");

        let laptop = Arc::new(laptop);
        let laptop_shared = Arc::new(Mutex::new(Quiet::default()));
        let mut heard = laptop.connection().subscribe();
        let mut laptops = Replicas::new(test_modules(be_block_client::Clock::manual()).await);
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
            step_laptop(&laptop, &laptop_shared, &mut laptops, &mut heard).await;
        }
        let mut laptop_mirror =
            attach_mirror::<TextContent>(&laptop, &laptop_shared, &mut laptops, block, 7).await;

        replicas
            .open((&peer, &shared), block, TextContent::CONTENT_TYPE, true)
            .await
            .expect("this device opens the text");
        step_until_replicas(&peer, &shared, &mut replicas, "joined", |replicas, _| {
            !replicas.joining() && replicas.status(block).is_some()
        })
        .await;
        let mut mirror =
            attach_mirror::<TextContent>(&peer, &shared, &mut replicas, block, 8).await;

        type_at_start(
            &mut laptops,
            block,
            7,
            &mut laptop_mirror,
            "socks\nshoes\nhat\n",
        );
        let socks = sealed_past(
            &laptop,
            &laptop_shared,
            &mut laptops,
            &mut heard,
            block,
            None,
        )
        .await;
        step_until_replicas(
            &peer,
            &shared,
            &mut replicas,
            "on the laptop's seal, fetched",
            |replicas, _| {
                !replicas.fetching_seals()
                    && replicas
                        .status(block)
                        .is_some_and(|status| status.head == Some(socks))
            },
        )
        .await;

        let (connection, token) = (peer.connection(), peer.token());
        peer.detach();
        replicas.disconnected();
        receive_frames(&shared, &mut replicas, block, 8, &mut mirror);
        type_at_start(&mut replicas, block, 8, &mut mirror, "boots\n");
        replicas.pump((&peer, &shared)).await;
        step_until_replicas(&peer, &shared, &mut replicas, "journaled", |replicas, _| {
            replicas
                .status(block)
                .is_some_and(|status| status.journaled)
        })
        .await;
        replicas.keep_all((&peer, &shared)).await;
        let offline = Arc::clone(&peer);
        step_until_replicas(
            &peer,
            &shared,
            &mut replicas,
            "sealed offline, on the laptop's seal",
            |_, shared| {
                !offline.pending_commits(block).is_empty()
                    || !shared.lock().unwrap().errors.is_empty()
            },
        )
        .await;
        let errors = std::mem::take(&mut shared.lock().unwrap().errors);
        assert!(errors.is_empty(), "{errors:?}");

        receive_frames(&laptop_shared, &mut laptops, block, 7, &mut laptop_mirror);
        let shown = laptop_mirror.content().expect("the laptop shows the text");
        let hat = "socks\nshoes\n".len()..TextBlock::text(shown).len();
        let deleted = TextBlock::delete(shown, hat).expect("there is a hat to delete");
        let message = laptop_mirror
            .operate(None, deleted)
            .expect("the laptop's editor has content");
        laptops.relay_from_editor(block, 7, message.encode(), std::time::Instant::now());
        let theirs = sealed_past(
            &laptop,
            &laptop_shared,
            &mut laptops,
            &mut heard,
            block,
            Some(socks),
        )
        .await;
        assert_ne!(theirs, socks);
        // The server prunes the seal the new one replaces in its slot: it holds it no more.
        let objects = SERVER_OBJECTS.take().expect("the server started");
        be_block_store::FileStore::open(&objects)
            .expect("the server's objects open")
            .remove(socks.hash())
            .expect("the server lets the seal go");
        SERVER_OBJECTS.set(Some(objects));

        peer.attach(connection, token)
            .await
            .expect("the device is connected again");
        replicas.reconnect((&peer, &shared));
        step_until_replicas(&peer, &shared, &mut replicas, "joined", |replicas, _| {
            !replicas.joining() && replicas.status(block).is_some()
        })
        .await;
        drain_all(&peer, &shared, &mut replicas, block).await;
        assert_eq!(
            text_on_the_server(&peer, block).await,
            "boots\nsocks\nshoes\n",
            "the laptop's deletion stands beside the edit made offline, with no conflict"
        );
        let errors = std::mem::take(&mut shared.lock().unwrap().errors);
        assert!(errors.is_empty(), "{errors:?}");
    });
}
