use std::{path::PathBuf, time::Duration};

use be_block::{BlockContent, ContentError};
use be_commit::{RetentionPolicy, retention::MINUTE};
use be_graph::BlockParent;
use be_store::{ChunkerConfig, MemoryStore};
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;

use super::*;
use image_block_types::{ImageContent, ImageHeader};

mod a_block_made_offline_takes_the_access_the_server_will_give_it;
mod a_block_a_shared_one_links_to_shows_only_that_it_exists;
mod a_block_moved_to_another_workspace_is_rewrapped_and_given_a_fresh_key;
mod a_connect_nobody_answers_times_out;
mod a_device_that_starts_offline_reads_and_lists_what_it_saved;
mod a_file_added_to_a_shared_folder_opens_and_saves_for_who_it_was_shared_with;
mod a_folder_shared_with_a_contact_opens_for_them_alone;
mod a_key_the_server_labels_as_the_organizations_settings_never_seals_another_block;
mod a_key_the_server_or_a_stranger_seals_to_a_member_is_never_used;
mod a_large_image_streams_without_downloading_all_of_it;
mod a_link_carrying_access_is_refused_where_nothing_is_shared_outside;
mod a_reader_cannot_read_a_block_moved_out_of_what_they_were_shared;
mod a_reader_taken_off_a_workspace_cannot_read_what_is_written_after;
mod a_reordered_reference_list_keeps_its_new_order;
mod a_request_the_server_never_answers_times_out;
mod a_save_keeps_defined_metadata_it_does_not_know_and_drops_stale_derived_metadata;
mod a_second_administrator_edits_the_organizations_settings;
mod a_stale_save_is_rejected_with_the_head_to_merge_against;
mod a_summary_read_before_a_rotation_never_brings_back_the_old_key;
mod a_workspace_not_kept_for_offline_use_is_forgotten_but_its_unsent_edits_stay;
mod an_administrator_reads_a_workspace_nobody_shared_with_them;
mod an_artifact_is_a_backref_of_the_block_it_describes;
mod an_editor_rotates_a_workspace_a_reader_was_taken_off;
mod an_escrow_key_the_server_puts_in_place_is_refused;
mod an_exported_artifact_imports_pointing_to_its_imported_source;
mod an_image_round_trips_without_being_re_encoded;
mod a_policy_outside_the_organizations_settings_is_not_enforced;
mod an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server;
mod contacts_keep_sharing_after_one_of_them_takes_a_new_recovery_phrase;
mod contents_read_together_match_what_each_block_saved;
mod edits_sealed_offline_are_published_when_the_device_reconnects;
mod edits_to_a_block_whose_access_was_lost_wait_aside_until_it_returns;
mod history_is_thinned_but_the_head_and_bookmarks_survive;
mod leaving_a_shared_workspace_leaves_its_rotation_to_who_can_edit_it;
mod metadata_is_sealed_on_the_server_and_opened_by_members;
mod metadata_that_does_not_decode_is_an_error_rather_than_blank;
mod moving_a_block_to_the_top_follows_who_may_make_workspaces;
mod references_declared_by_content_reach_the_graph;
mod relayed_messages_reach_one_peer_or_all_of_them_sealed;
mod removing_a_member_rotates_the_organizations_settings;
mod removing_a_member_shuts_them_out_and_rotates_what_they_could_read;
mod swapping_ids_replaces_every_written_form_of_each;
mod two_peers_of_one_account_read_each_others_blocks;
mod what_a_policy_governs_waits_while_the_server_cannot_read_it;

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
            &admin.connection(),
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
        crate::contacts::accept_invitation(
            &own.connection(),
            own.keys().identity(),
            &theirs,
            &code,
        )
        .await
        .unwrap();
        crate::contacts::finish_links(
            &admin.connection(),
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

    fn config(&self, like: &Peer<MemoryStore>, refs: &refs::RefsDb) -> PeerConfig {
        PeerConfig::new(
            &self.url,
            like.keys().identity().clone(),
            Credentials::Token(like.token()),
        )
        .organization(Some(like.organization()))
        .chunker(ChunkerConfig::SMALL)
        .refs(refs.clone())
    }

    async fn device(
        &self,
        like: &Peer<MemoryStore>,
        refs: &refs::RefsDb,
        store: &MemoryStore,
    ) -> Peer<MemoryStore> {
        Peer::connect(self.config(like, refs), store.clone())
            .await
            .unwrap()
    }

    fn offline(
        &self,
        like: &Peer<MemoryStore>,
        refs: &refs::RefsDb,
        store: &MemoryStore,
    ) -> Peer<MemoryStore> {
        Peer::cached(self.config(like, refs), like.account(), store.clone()).unwrap()
    }

    async fn reconnect(&self, peer: &Peer<MemoryStore>, token: String) {
        let connection = Connection::connect(&self.url, &Clock::real())
            .await
            .unwrap();
        let be_protocol::ServerMessage::Authenticated { token, .. } = connection
            .request(|request| be_protocol::ClientMessage::Authenticate { request, token })
            .await
            .unwrap()
        else {
            panic!("the server did not sign the device in");
        };
        peer.attach(connection, token).await.unwrap();
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
        workspace_ui_types::WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        &be_block::BlockMetadata::named(name),
    )
    .await
    .unwrap()
    .id
}

async fn my_files(peer: &Peer<MemoryStore>) -> Uuid {
    peer.create_block(
        be_block::my_files(peer.organization(), peer.account()),
        workspace_ui_types::WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        &be_block::BlockMetadata::named("My files"),
    )
    .await
    .unwrap()
    .id
}

async fn text(peer: &Peer<MemoryStore>, block: Uuid) -> String {
    let opened = peer
        .open::<text_block_types::TextContent>(block)
        .await
        .unwrap()
        .expect("the block has content");
    text_block_types::TextBlock::text(&opened)
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

const PATIENCE: Duration = Duration::from_secs(20);
