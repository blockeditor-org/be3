use std::{path::PathBuf, sync::Arc, time::Duration};

use be_block::{BlockContent, ContentError, ImageContent, ImageHeader};
use be_commit::{RetentionPolicy, retention::MINUTE};
use be_graph::BlockParent;
use be_store::{ChunkerConfig, MemoryStore};
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;

use super::*;

mod a_block_a_shared_one_links_to_shows_only_that_it_exists;
mod a_block_moved_to_another_workspace_is_rewrapped_and_given_a_fresh_key;
mod a_file_added_to_a_shared_folder_opens_and_saves_for_who_it_was_shared_with;
mod a_folder_shared_with_a_contact_opens_for_them_alone;
mod a_follower_keeps_its_unconfirmed_typing_through_a_reload;
mod a_follower_replaces_an_image_and_the_owner_takes_it;
mod a_follower_stops_saving_after_a_message_it_cannot_read;
mod a_follower_takes_over_and_keeps_editing_without_a_merge;
mod a_follower_that_falls_behind_after_following_keeps_what_it_had;
mod a_follower_that_falls_behind_catches_up;
mod a_follower_that_takes_over_keeps_what_it_typed_before_its_first_save;
mod a_follower_typing_while_the_owner_types_keeps_both_runs_whole;
mod a_key_the_server_or_a_stranger_seals_to_a_member_is_never_used;
mod a_large_image_streams_without_downloading_all_of_it;
mod a_late_joiner_places_an_insert_after_an_item_removed_before_it_joined;
mod a_late_joiner_to_edited_text_types_where_it_sees_the_caret;
mod a_peer_edit_reaching_a_follower_mid_gesture_goes_beneath_its_own;
mod a_reader_taken_off_a_workspace_cannot_read_what_is_written_after;
mod a_rename_and_an_unsaved_edit_both_reach_the_head;
mod a_reordered_reference_list_keeps_its_new_order;
mod a_save_keeps_defined_metadata_it_does_not_know_and_drops_stale_derived_metadata;
mod a_second_follower_keeps_following_after_the_owner_leaves;
mod a_stale_save_is_rejected_with_the_head_to_merge_against;
mod a_summary_read_before_a_rotation_never_brings_back_the_old_key;
mod a_text_session_absorbs_a_file_written_by_be_sync_while_typing;
mod an_administrator_reads_a_workspace_nobody_shared_with_them;
mod an_edit_too_large_to_relay_is_saved_as_a_commit;
mod an_escrow_key_the_server_puts_in_place_is_refused;
mod an_idle_session_catches_up_with_a_publish_from_outside_it;
mod an_image_round_trips_without_being_re_encoded;
mod an_offline_edit_elsewhere_merges_cleanly;
mod an_offline_rewrite_conflicts_instead_of_interleaving;
mod an_unsaved_edit_survives_a_publish_from_outside_the_session;
mod an_unsaved_rewrite_that_conflicts_keeps_both_versions_in_history;
mod channel_messages_reach_one_peer_or_all_of_them_sealed;
mod contacts_keep_sharing_after_one_of_them_takes_a_new_recovery_phrase;
mod contents_read_together_match_what_each_block_saved;
mod history_is_thinned_but_the_head_and_bookmarks_survive;
mod large_pastes_behind_an_unconfirmed_edit_go_out_in_relayable_pieces;
mod leaving_a_shared_workspace_leaves_its_rotation_to_who_can_edit_it;
mod metadata_is_sealed_on_the_server_and_opened_by_members;
mod metadata_that_does_not_decode_is_an_error_rather_than_blank;
mod presence_reaches_late_joiners_and_leaves_with_its_client;
mod references_declared_by_content_reach_the_graph;
mod removing_a_member_shuts_them_out_and_rotates_what_they_could_read;
mod swapping_ids_replaces_every_written_form_of_each;
mod the_journal_says_how_each_peer_saw_its_content_change;
mod two_peers_converge_through_the_session_owner;
mod two_peers_of_one_account_read_each_others_blocks;
mod typing_behind_an_unconfirmed_edit_goes_out_as_one_edit;

struct Harness {
    url: String,
    directory: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Harness {
    async fn start() -> Self {
        build_id::install("test");
        let directory = std::env::temp_dir().join(format!("be-client-test-{}", Uuid::new_v4()));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, receiver) = oneshot::channel();
        let data_dir = directory.clone();
        let handle = tokio::spawn(async move {
            let _ = be_server::serve_with_config(
                listener,
                data_dir,
                be_server::ServerConfig::OPEN,
                receiver,
            )
            .await;
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
                identity(email),
                Credentials::Token(
                    be_server::ServerStore::open(&self.directory)
                        .unwrap()
                        .register(email, email, "correct horse battery")
                        .await
                        .unwrap()
                        .1,
                ),
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
                owner.keys().identity().clone(),
                Credentials::Token(owner.token().to_owned()),
            )
            .organization(Some(owner.organization()))
            .chunker(ChunkerConfig::SMALL),
            MemoryStore::new(),
        )
        .await
        .unwrap()
    }

    async fn member(&self, admin: &Peer<MemoryStore>, email: &str) -> Peer<MemoryStore> {
        self.member_pinning(admin, email, admin.keys().escrow().as_ref())
            .await
    }

    async fn member_pinning(
        &self,
        admin: &Peer<MemoryStore>,
        email: &str,
        escrow: Option<&be_protocol::EscrowKey>,
    ) -> Peer<MemoryStore> {
        let own = self.owner(email).await;
        let card = crate::contacts::card(
            admin.account(),
            admin.keys().identity(),
            "Admin",
            "admin@example.com",
        );
        let code = crate::contacts::invite(
            admin.connection(),
            admin.keys().identity(),
            &card,
            admin.organization(),
            email.into(),
            be_protocol::OrganizationRole::Editor,
            escrow,
        )
        .await
        .unwrap();
        let theirs = crate::contacts::card(own.account(), own.keys().identity(), email, email);
        crate::contacts::accept_invitation(own.connection(), own.keys().identity(), &theirs, &code)
            .await
            .unwrap();
        crate::contacts::finish_links(
            admin.connection(),
            admin.keys().identity(),
            admin.organization(),
        )
        .await
        .unwrap();
        admin.refresh_contacts().await.unwrap();
        Peer::connect(
            PeerConfig::new(
                &self.url,
                own.keys().identity().clone(),
                Credentials::Token(own.token().to_owned()),
            )
            .organization(Some(admin.organization()))
            .chunker(ChunkerConfig::SMALL),
            MemoryStore::new(),
        )
        .await
        .unwrap()
    }

    fn database(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.directory.join("metadata.sqlite")).unwrap()
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

async fn workspace(peer: &Peer<MemoryStore>, name: &str) -> Uuid {
    peer.create_block(
        Uuid::now_v7(),
        be_block::WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        &be_block::BlockMetadata::named(name),
    )
    .await
    .unwrap()
    .id
}

async fn text(peer: &Peer<MemoryStore>, block: Uuid) -> String {
    let opened = peer
        .open::<be_block::TextContent>(block)
        .await
        .unwrap()
        .expect("the block has content");
    be_block::TextBlock::text(&opened)
}

fn identity(email: &str) -> be_keys::RecoverySecret {
    let mut bytes = [7u8; 32];
    for (index, byte) in email.bytes().enumerate() {
        bytes[index % 32] = bytes[index % 32].wrapping_mul(31).wrapping_add(byte);
    }
    be_keys::RecoverySecret::from_bytes(bytes)
}

fn image(name: &str, bytes: usize, seed: u64) -> ImageContent {
    ImageContent::new(
        ImageHeader {
            source_name: name.into(),
            media_type: "image/png".into(),
            width: 1920,
            height: 1080,
            failure: None,
            thumbhash: None,
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

fn checklist_texts(live: &Live<MemoryStore, be_block::ChecklistContent>) -> Vec<String> {
    live.content()
        .root()
        .items
        .iter()
        .map(|item| item.text.clone())
        .collect()
}

trait Texted {
    fn to_text(&self) -> String;
}

impl Texted for be_block::TextContent {
    fn to_text(&self) -> String {
        be_block::TextBlock::text(self)
    }
}

async fn type_at(live: &mut Live<MemoryStore, be_block::TextContent>, at: usize, typed: &str) {
    let edit = be_block::TextBlock::insert(live.content(), live.client(), at, typed.as_bytes())
        .expect("there is somewhere to type");
    live.edit(edit).await.unwrap();
}

async fn erase(live: &mut Live<MemoryStore, be_block::TextContent>, range: std::ops::Range<usize>) {
    let edit =
        be_block::TextBlock::delete(live.content(), range).expect("there is something to erase");
    live.edit(edit).await.unwrap();
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
