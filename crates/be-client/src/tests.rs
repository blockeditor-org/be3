use std::{path::PathBuf, sync::Arc, time::Duration};

use be_block::{BlockContent, ContentError, ImageContent, ImageHeader};
use be_commit::{RetentionPolicy, retention::MINUTE};
use be_graph::BlockParent;
use be_store::{ChunkerConfig, ContentKey, MemoryStore};
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;

use super::*;

mod a_follower_takes_over_and_keeps_editing_without_a_merge;
mod a_follower_that_takes_over_keeps_what_it_typed_before_its_first_save;
mod a_large_image_streams_without_downloading_all_of_it;
mod a_second_follower_keeps_following_after_the_owner_leaves;
mod a_stale_save_is_rejected_with_the_head_to_merge_against;
mod an_image_round_trips_without_being_re_encoded;
mod an_offline_edit_elsewhere_merges_cleanly;
mod an_offline_rewrite_conflicts_instead_of_interleaving;
mod an_unsaved_edit_survives_a_publish_from_outside_the_session;
mod history_is_thinned_but_the_head_and_bookmarks_survive;
mod references_declared_by_content_reach_the_graph;
mod two_peers_converge_through_the_session_owner;

struct Harness {
    url: String,
    directory: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Harness {
    async fn start() -> Self {
        let directory = std::env::temp_dir().join(format!("be-client-test-{}", Uuid::new_v4()));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, receiver) = oneshot::channel();
        let data_dir = directory.clone();
        let handle = tokio::spawn(async move {
            let _ = be_server::serve_until_shutdown(listener, data_dir, receiver).await;
        });
        Self {
            url: format!("ws://{address}"),
            directory,
            shutdown: Some(shutdown),
            handle: Some(handle),
        }
    }

    async fn shared(&self, owner: &Peer<MemoryStore>) -> Arc<Peer<MemoryStore>> {
        Arc::new(self.second(owner).await)
    }

    async fn owner(&self, email: &str) -> Peer<MemoryStore> {
        Peer::connect(
            PeerConfig::new(
                &self.url,
                ContentKey::from_bytes([11; 32]),
                Credentials::Register {
                    email: email.into(),
                    display_name: email.into(),
                    password: "correct horse battery".into(),
                },
            )
            .chunker(ChunkerConfig::SMALL),
            MemoryStore::new(),
        )
        .await
        .unwrap()
    }

    async fn second(&self, owner: &Peer<MemoryStore>) -> Peer<MemoryStore> {
        Peer::connect(
            PeerConfig::new(
                &self.url,
                ContentKey::from_bytes([11; 32]),
                Credentials::Token(owner.token().to_owned()),
            )
            .workspace(Some(owner.workspace()))
            .chunker(ChunkerConfig::SMALL),
            MemoryStore::new(),
        )
        .await
        .unwrap()
    }

    async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            handle.abort();
            let _ = handle.await;
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn image(name: &str, bytes: usize, seed: u64) -> ImageContent {
    ImageContent::new(
        ImageHeader {
            source_name: name.into(),
            media_type: "image/png".into(),
            width: 1920,
            height: 1080,
        },
        pseudorandom(bytes, seed),
    )
}

fn pseudorandom(length: usize, seed: u64) -> Vec<u8> {
    let mut state = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    (0..length)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            u8::try_from((state >> 33) & 0xff).unwrap()
        })
        .collect()
}

struct Link {
    targets: Vec<Uuid>,
}

impl BlockContent for Link {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x11_11);

    fn encode(&self) -> Vec<u8> {
        self.targets
            .iter()
            .flat_map(|target| target.into_bytes())
            .collect()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        if !bytes.len().is_multiple_of(16) {
            return Err(ContentError::Malformed("link list"));
        }
        Ok(Self {
            targets: bytes
                .as_chunks::<16>()
                .0
                .iter()
                .map(|chunk| Uuid::from_slice(chunk).expect("sixteen bytes"))
                .collect(),
        })
    }

    fn references(&self) -> Vec<Uuid> {
        self.targets.clone()
    }
}

const PATIENCE: Duration = Duration::from_secs(20);

async fn until<C: be_block::LiveEdit + Clone + Default>(
    sessions: &mut [&mut Live<MemoryStore, C>],
    what: &str,
    ready: impl Fn(&[&mut Live<MemoryStore, C>]) -> bool,
) {
    let reached = tokio::time::timeout(PATIENCE, async {
        loop {
            for session in sessions.iter_mut() {
                session.poll().await.unwrap();
            }
            if ready(sessions) {
                return;
            }
            let waits = sessions.iter_mut().map(|session| Box::pin(session.wait()));
            futures_util::future::select_all(waits).await.0.unwrap();
        }
    })
    .await;
    assert!(reached.is_ok(), "the sessions never {what}");
}

async fn settle<C: be_block::LiveEdit + Clone + Default>(
    sessions: &mut [&mut Live<MemoryStore, C>],
) {
    for _ in 0..12 {
        let mut handled = 0;
        for session in sessions.iter_mut() {
            handled += session.poll().await.unwrap();
        }
        if handled == 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            let mut again = 0;
            for session in sessions.iter_mut() {
                again += session.poll().await.unwrap();
            }
            if again == 0 {
                return;
            }
        }
    }
}
