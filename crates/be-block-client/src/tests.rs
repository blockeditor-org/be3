use be_block::{BlockContent, ContentError};
use be_block_graph::BlockParent;
use be_block_store::{ChunkerConfig, MemoryStore};
use be_server::Running;
use uuid::Uuid;

use super::*;
use be_type_image::{ImageContent, ImageHeader};

/// Uploads `objects` as `Peer::push` does, without keeping them on the device: asks for URLs,
/// then PUTs each object's bytes.
async fn upload(peer: &Peer<MemoryStore>, objects: Vec<Vec<u8>>) -> Result<(), ClientError> {
    let lengths = objects
        .iter()
        .map(|bytes| be_block_protocol::ObjectLength {
            hash: be_block_store::Hash::of(bytes),
            length: bytes.len() as u64,
        })
        .collect();
    let urls = crate::ask(be_block_protocol::client::objects::upload_urls(
        &peer.connection(),
        lengths,
    ))
    .await?;
    for (url, bytes) in urls.into_iter().zip(objects) {
        let headers: Vec<(String, String)> = url
            .headers
            .into_iter()
            .map(|header| (header.name, header.value))
            .collect();
        crate::http::put(&url.url, &headers, bytes)
            .await
            .map_err(|error| ClientError::Disconnected(error.to_string()))?;
    }
    Ok(())
}

mod a_block_a_shared_one_links_to_shows_only_that_it_exists;
mod a_block_deleted_after_its_folder_was_listed_is_listed_only_as_deleted_offline;
mod a_block_made_offline_keeps_its_edits_when_read_before_it_is_sent;
mod a_block_made_offline_takes_the_access_the_server_will_give_it;
mod a_block_moved_out_of_a_listed_folder_is_listed_where_it_went_offline;
mod a_block_moved_to_another_workspace_is_rewrapped_and_given_a_fresh_key;
mod a_block_placed_in_a_folder_after_it_was_listed_is_listed_there_offline;
mod a_block_the_server_refused_to_make_is_not_listed_offline;
mod a_change_the_server_refuses_leaves_the_rest_of_the_outbox_to_send;
mod a_connect_nobody_answers_times_out;
mod a_connection_outlives_a_stall_of_this_process;
mod a_creation_the_server_refuses_is_set_aside_and_nothing_waits_behind_it;
mod a_definition_the_server_refuses_leaves_the_block_as_the_server_has_it;
mod a_device_that_starts_offline_reads_and_lists_what_it_saved;
mod a_file_added_to_a_shared_folder_opens_and_saves_for_who_it_was_shared_with;
mod a_folder_listed_again_answers_offline_as_of_the_newer_listing;
mod a_folder_shared_with_a_contact_opens_for_them_alone;
mod a_key_the_server_labels_as_the_organizations_settings_never_seals_another_block;
mod a_key_the_server_or_a_stranger_seals_to_a_member_is_never_used;
mod a_large_block_saved_after_a_small_change_uploads_only_what_changed;
mod a_large_image_streams_without_downloading_all_of_it;
mod a_link_carrying_access_is_refused_where_nothing_is_shared_outside;
mod a_listed_block_this_device_may_no_longer_know_of_is_left_out_offline;
mod a_merge_left_waiting_is_sent_once_the_device_reconnects;
mod a_move_refused_to_an_editor_drops_only_the_move;
mod a_notification_opens_only_for_the_blocks_members;
mod a_part_put_for_an_operation_is_saved_by_another_member_without_uploading_it;
mod a_policy_outside_the_organizations_settings_is_not_enforced;
mod a_reader_cannot_read_a_block_moved_out_of_what_they_were_shared;
mod a_reader_taken_off_a_workspace_cannot_read_what_is_written_after;
mod a_real_clock_places_an_instant_ahead_of_now_that_far_ahead;
mod a_reordered_reference_list_keeps_its_new_order;
mod a_request_fails_once_a_ping_goes_unanswered;
mod a_request_on_a_link_gone_silent_fails_without_waiting_for_the_idle_ping;
mod a_request_stuck_in_a_full_socket_fails_once_the_server_goes_silent;
mod a_save_keeps_defined_metadata_it_does_not_know_and_drops_stale_derived_metadata;
mod a_saved_record_that_does_not_decode_leaves_the_rest_readable;
mod a_second_administrator_edits_the_organizations_settings;
mod a_signature_is_verified_by_a_contacts_key_pinned_from_the_server_or_refused;
mod a_stale_card_does_not_make_a_replaced_signing_key_current;
mod a_stale_save_is_rejected_with_the_head_to_merge_against;
mod a_summary_read_before_a_rotation_never_brings_back_the_old_key;
mod a_watch_the_server_ended_comes_back_with_its_holders_when_the_view_returns;
mod a_workspace_not_kept_for_offline_use_is_forgotten_but_its_unsent_edits_stay;
mod an_administrator_reads_a_workspace_nobody_shared_with_them;
mod an_artifact_is_a_backref_of_the_block_it_describes;
mod an_edit_the_server_kept_without_answering_settles_once_another_device_builds_on_it;
mod an_editor_rotates_a_workspace_a_reader_was_taken_off;
mod an_escrow_key_the_server_puts_in_place_is_refused;
mod an_exported_artifact_imports_pointing_to_its_imported_source;
mod an_image_round_trips_without_being_re_encoded;
mod an_offline_edit_merges_against_the_head_it_left_after_the_server_pruned_it;
mod an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server;
mod an_owner_only_block_is_read_by_its_author_alone;
mod collecting_a_removed_members_settings_takes_what_only_they_read;
mod contacts_keep_sharing_after_one_of_them_takes_a_new_recovery_phrase;
mod contents_read_together_match_what_each_block_saved;
mod edits_sealed_offline_are_published_when_the_device_reconnects;
mod edits_to_a_block_whose_access_was_lost_wait_aside_until_it_returns;
mod history_is_thinned_but_the_head_and_bookmarks_survive;
mod large_files_saved_at_once_through_a_relay_all_arrive;
mod leaving_a_shared_workspace_leaves_its_rotation_to_who_can_edit_it;
mod metadata_is_sealed_on_the_server_and_opened_by_members;
mod metadata_that_does_not_decode_is_an_error_rather_than_blank;
mod moving_a_block_to_the_top_follows_who_may_make_workspaces;
mod objects_move_between_devices_and_a_bucket_directly;
mod objects_move_through_the_servers_own_s3_endpoint;
mod references_declared_by_content_reach_the_graph;
mod relayed_messages_reach_one_peer_or_all_of_them_sealed;
mod removing_a_member_rotates_a_workspaces_search_index_with_it;
mod removing_a_member_rotates_the_organizations_settings;
mod removing_a_member_shuts_them_out_and_rotates_what_they_could_read;
mod swapping_ids_replaces_every_written_form_of_each;
mod two_peers_of_one_account_read_each_others_blocks;
mod what_a_policy_governs_waits_while_the_server_cannot_read_it;
mod what_is_in_force_at_a_block_not_sent_yet_is_answered_from_this_device;
mod what_is_made_in_a_block_the_server_refused_waits_aside_with_it;

struct Harness {
    server: Running,
    url: String,
}

impl Harness {
    async fn start() -> Self {
        Self::configured(be_server::ServerConfig::OPEN).await
    }

    async fn configured(config: be_server::ServerConfig) -> Self {
        be_lib_build_id::install("test");
        let server = Running::start(config).await;
        Self {
            url: format!("ws://{}", server.address),
            server,
        }
    }

    async fn owner(&self, email: &str) -> Peer<MemoryStore> {
        Peer::connect(
            PeerConfig::new(
                &self.url,
                identity(email),
                Credentials::Token(self.server.register(email, email).await),
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
        escrow: Option<&be_block_protocol::EscrowKey>,
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
            be_block_protocol::OrganizationRole::Editor,
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
        let token = crate::ask(be_block_protocol::client::account::authenticate(
            &connection,
            Some(token),
        ))
        .await
        .expect("the server signs the device in")
        .token;
        peer.attach(connection, token).await.unwrap();
    }

    fn database(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self.server.directory.join("metadata.sqlite")).unwrap()
    }

    async fn stop(self) {
        self.server.stop().await;
    }
}

async fn workspace(peer: &Peer<MemoryStore>, name: &str) -> Uuid {
    peer.create_block(
        Uuid::now_v7(),
        be_type_workspace::WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        &be_block::BlockMetadata::named(name),
        false,
    )
    .await
    .unwrap()
    .id
}

async fn my_files(peer: &Peer<MemoryStore>) -> Uuid {
    peer.create_block(
        be_block::my_files(peer.organization(), peer.account()),
        be_type_workspace::WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        &be_block::BlockMetadata::named("My files"),
        false,
    )
    .await
    .unwrap()
    .id
}

async fn text(peer: &Peer<MemoryStore>, block: Uuid) -> String {
    let opened = peer
        .open::<be_type_text::TextContent>(block)
        .await
        .unwrap()
        .expect("the block has content");
    be_type_text::TextBlock::text(&opened)
}

/// A device on `harness` with the default chunker, so a large text is a few large objects.
async fn device(harness: &Harness, email: &str) -> Peer<MemoryStore> {
    Peer::connect(
        PeerConfig::new(
            &harness.url,
            identity(email),
            Credentials::Token(harness.server.register(email, email).await),
        ),
        MemoryStore::new(),
    )
    .await
    .unwrap()
}

/// Another device of `owner`'s account.
async fn another(harness: &Harness, owner: &Peer<MemoryStore>) -> Peer<MemoryStore> {
    Peer::connect(
        PeerConfig::new(
            &harness.url,
            owner.keys().identity().clone(),
            Credentials::Token(owner.token().to_owned()),
        )
        .organization(Some(owner.organization())),
        MemoryStore::new(),
    )
    .await
    .unwrap()
}

/// A text block in a workspace of its own.
async fn note(owner: &Peer<MemoryStore>) -> Uuid {
    let workspace = owner
        .create_block(
            Uuid::now_v7(),
            be_type_workspace::WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block::BlockMetadata::named("Notes"),
            false,
        )
        .await
        .unwrap()
        .id;
    owner
        .create::<be_type_text::TextContent>(BlockParent::Block(workspace))
        .await
        .unwrap()
}

/// A text too large to travel inside a publish, so its objects move through URLs.
fn large_text() -> String {
    (0..400_000).map(|line| format!("line {line}\n")).collect()
}

fn identity(email: &str) -> be_block_keys::RecoverySecret {
    let mut bytes = [7u8; 32];
    for (index, byte) in email.bytes().enumerate() {
        bytes[index % 32] = bytes[index % 32].wrapping_mul(31).wrapping_add(byte);
    }
    be_block_keys::RecoverySecret::from_bytes(bytes)
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

/// Refs for one account in one organization, kept in `db`: scoping `db` again reads them back as a
/// device does when it starts.
fn scoped(db: &refs::RefsDb) -> refs::Refs {
    db.scope(refs::Scope {
        server: "http://server".to_owned(),
        account: Uuid::from_u128(1),
        organization: Uuid::from_u128(2),
    })
    .unwrap()
}

/// A block's summary as the server would tell it, placed under `parent` as of `revision`.
fn placed(id: u128, parent: BlockParent, revision: u64) -> be_block_protocol::BlockSummary {
    be_block_protocol::BlockSummary {
        id: Uuid::from_u128(id),
        content_type: Uuid::from_u128(0x11_11),
        access: be_block_graph::Access::Manage,
        author: None,
        parent: Some(parent),
        head: None,
        references: Vec::new(),
        metadata: Vec::new(),
        version: None,
        key: None,
        scope: None,
        revision,
        held: false,
        opens_inside: false,
        sets_access: false,
        size: 0,
        tree_size: 0,
        owner_only: false,
    }
}

/// The ids a listing answers offline.
fn offline_ids(refs: &refs::Refs, listing: be_block_protocol::Listing) -> Option<Vec<Uuid>> {
    refs.cached_listing(listing)
        .map(|blocks| blocks.iter().map(|summary| summary.id).collect())
}
