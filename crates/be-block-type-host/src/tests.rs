use std::sync::Arc;

use be_block_client::{ClientError, Credentials, Peer, PeerConfig};
use be_block_commit::CommitId;
use be_block_content::BlockContent;
use be_block_graph::BlockParent;
use be_block_store::{Manifest, MemoryStore};
use be_sdk_type_api::{Description, EditorId, Status};
use be_sdk_type_testing::probe::{self, Probe};
use be_sdk_type_testing::sections::{self, Sections};
use be_wasm_threads::Mutex;
use uuid::Uuid;

use crate::{Conflict, Host, Replicas, in_process, started};

mod a_block_sent_while_its_replica_owes_a_seal_is_asked_for_it_once;
mod a_block_whose_creation_waits_joins_once_it_is_sent_not_on_reconnecting;
mod a_journal_is_opened_on_the_commit_a_seal_was_made_on_only_for_this_devices_noted_seal;
mod a_key_searched_for_in_vain_is_searched_for_again_after_reconnecting;
mod a_merge_over_parts_reads_only_the_parts_each_side_changed;
mod a_modules_answer_is_seen_after_the_host_task_spent_its_budget;
mod a_relayed_message_sealed_with_a_key_another_search_awaits_waits_for_it;
mod a_stateless_request_reads_only_parts_of_the_commits_it_was_handed;
mod a_stream_of_relays_under_keys_that_cannot_be_got_costs_one_search_per_key;
mod an_edit_and_a_definition_kept_beside_another_devices_head_both_reach_the_server;
mod an_edit_made_on_the_content_from_before_a_replica_joins_lands_after;
mod an_edit_from_an_editor_that_left_before_its_block_joined_is_kept;
mod an_edit_sealed_beside_a_commit_waiting_in_the_outbox_is_merged_with_it;
mod an_edit_sent_after_its_replica_made_a_clean_status_is_given_again_on_joining;
mod news_of_a_block_whose_head_stayed_moves_no_replica;
mod search_text_of_a_type_that_loads_whole_is_narrowed_to_the_keys_asked_for;
mod search_text_reads_only_the_parts_asked_for_and_not_yet_indexed;
mod two_hosts_in_one_process_each_send_a_type_to_their_own_module;
mod what_a_replica_opened_offline_replayed_from_its_journal_survives_rejoining;
mod what_a_wait_on_a_module_handles_is_taken_as_a_change;

/// A host that keeps nothing but which editors it was given frames for, for tests of stateless
/// requests.
#[derive(Default)]
struct Quiet {
    to_editors: Vec<EditorId>,
    /// What each editor was sent, in order.
    frames: Vec<(EditorId, Vec<u8>)>,
    /// What was recorded as failing.
    errors: Vec<String>,
}

impl Host for Quiet {
    type Store = MemoryStore;

    fn spawn(task: impl Future<Output = ()> + 'static) {
        tokio::task::spawn_local(task);
    }

    fn wake() {}

    fn record(shared: &Arc<Mutex<Self>>, error: ClientError) {
        eprintln!("{error}");
        shared.lock().unwrap().errors.push(error.to_string());
    }

    fn record_sync(shared: &Arc<Mutex<Self>>, error: ClientError) {
        Self::record(shared, error);
    }

    fn report(_: &Arc<Mutex<Self>>, _: Option<Uuid>, message: String) {
        eprintln!("{message}");
    }

    fn set_locked(_: &Arc<Mutex<Self>>, _: Uuid, _: bool) {}

    fn owned(_: &Arc<Mutex<Self>>, _: Uuid) -> bool {
        false
    }

    fn reshaped(_: &Arc<Mutex<Self>>, _: Uuid, _: &str) -> bool {
        false
    }

    fn reset_notice() {}

    fn known_head(_: &Arc<Mutex<Self>>, _: Uuid) -> Option<CommitId> {
        None
    }

    fn to_editor(shared: &Arc<Mutex<Self>>, _: Uuid, editor: EditorId, frame: Vec<u8>) -> bool {
        let mut held = shared.lock().unwrap();
        held.to_editors.push(editor);
        held.frames.push((editor, frame));
        false
    }

    fn content(_: &Arc<Mutex<Self>>, _: Uuid, _: Uuid, _: Vec<u8>) {}

    fn released(_: &Arc<Mutex<Self>>, _: Uuid) {}

    fn status(_: &Arc<Mutex<Self>>, _: Uuid, _: &Status) {}

    fn described(_: &Arc<Mutex<Self>>, _: Uuid, _: Vec<Uuid>, _: Description) {}

    fn conflict(_: &Arc<Mutex<Self>>, _: Uuid, _: Conflict) {}

    fn counted(_: &Arc<Mutex<Self>>, _: usize, _: Vec<crate::Owing>, _: Vec<crate::Owing>) {}

    fn leaving(_: &Arc<Mutex<Self>>, _: bool) {}
}

/// Runs `test` with a server, a peer signed in to it and a host holding the `Sections`,
/// `Probe` and text types.
fn with_test_types<F: Future<Output = ()>>(
    test: impl FnOnce(Arc<Peer<MemoryStore>>, Arc<Mutex<Quiet>>, Replicas<Quiet>) -> F,
) {
    with_test_types_on(be_block_client::Clock::real(), test);
}

/// As `with_test_types`, its modules on `clock`.
fn with_test_types_on<F: Future<Output = ()>>(
    clock: be_block_client::Clock,
    test: impl FnOnce(Arc<Peer<MemoryStore>>, Arc<Mutex<Quiet>>, Replicas<Quiet>) -> F,
) {
    with_devices(false, clock, |peer, _, shared, replicas| {
        test(peer, shared, replicas)
    });
}

/// As `with_test_types`, with another device of the same account, signed in on a store of its
/// own.
fn with_another_device<F: Future<Output = ()>>(
    test: impl FnOnce(
        Arc<Peer<MemoryStore>>,
        Peer<MemoryStore>,
        Arc<Mutex<Quiet>>,
        Replicas<Quiet>,
    ) -> F,
) {
    with_devices(true, be_block_client::Clock::real(), |peer, another, shared, replicas| {
        test(
            peer,
            another.expect("another device signed in"),
            shared,
            replicas,
        )
    });
}

fn with_devices<F: Future<Output = ()>>(
    another: bool,
    clock: be_block_client::Clock,
    test: impl FnOnce(
        Arc<Peer<MemoryStore>>,
        Option<Peer<MemoryStore>>,
        Arc<Mutex<Quiet>>,
        Replicas<Quiet>,
    ) -> F,
) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts");
    tokio::task::LocalSet::new().block_on(&runtime, async {
        be_lib_build_id::install("test");
        let server = be_server::Running::start(be_server::ServerConfig::OPEN).await;
        let email = format!("{}@example.com", Uuid::new_v4().simple());
        let phrase = be_block_keys::RecoveryPhrase::generate();
        let token = server.register(&email, "Owner").await;
        let url = be_block_client::socket_url(&server.url);
        let device = |organization| {
            let config = PeerConfig::new(
                url.clone(),
                phrase.secret(),
                Credentials::Token(token.clone()),
            )
            .organization(organization);
            async move {
                Peer::connect(config, MemoryStore::new())
                    .await
                    .expect("the peer signs in")
            }
        };
        let peer = device(None).await;
        let another = match another {
            true => Some(device(Some(peer.organization())).await),
            false => None,
        };
        let modules = test_modules(clock).await;
        test(
            Arc::new(peer),
            another,
            Arc::new(Mutex::new(Quiet::default())),
            Replicas::new(modules),
        )
        .await;
        server.stop().await;
    });
}

/// The modules of the `Sections`, `Probe` and text types, on `clock`.
async fn test_modules(clock: be_block_client::Clock) -> crate::Modules {
    started(
        vec![
            in_process::<sections::BlockTypes>("test.sections"),
            in_process::<probe::BlockTypes>("test.probe"),
            in_process::<be_type_text::BlockTypes>("be3.text"),
        ],
        clock,
    )
    .await
}

/// The ten sections, `changed` ones replaced, as a part of the block of their own: the
/// manifest a commit of it would hold.
async fn written(peer: &Peer<MemoryStore>, block: Uuid, changed: &[(usize, &str)]) -> Manifest {
    let mut content = base();
    for (index, text) in changed {
        content.set(*index, text);
    }
    peer.write_content(block, Sections::CONTENT_TYPE, content.parts())
        .await
        .expect("the content is written")
}

fn base() -> Sections {
    Sections::of(&[
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ])
}

/// Each section's part in a manifest, as the type loads it.
async fn section_parts(
    peer: &Peer<MemoryStore>,
    manifest: &Manifest,
) -> Vec<be_block_store::PartRef> {
    let stored = peer
        .load_first_part(manifest)
        .await
        .expect("the first part loads");
    let content = Sections::from_stored(&stored).expect("the sections decode");
    (0..sections::SECTIONS)
        .map(|index| content.saved(index).expect("every section is a part"))
        .collect()
}

async fn block(peer: &Peer<MemoryStore>) -> Uuid {
    peer.create::<Sections>(BlockParent::Root)
        .await
        .expect("the block is made")
}

/// A block of `Sections` at a published head, with a definition naming it "Renamed" kept in the
/// outbox on that head and an edit of section 2 sealed beside it, on that head too, and handed
/// to `publish_sealed`: the block, its head, the definition and the edit.
async fn sealed_beside_a_definition(
    peer: &Peer<MemoryStore>,
) -> (Uuid, CommitId, CommitId, CommitId) {
    let block = block(peer).await;
    let head = match peer.save(block, &base(), None).await {
        Ok(be_block_client::Saved::Published(head)) => head,
        saved => panic!("the base publishes: {saved:?}"),
    };
    peer.keep_definition(block, |metadata| {
        metadata.insert(&be_block_content::NAME, &"Renamed".to_owned());
    })
    .await
    .expect("the definition is kept here");
    let definition = peer.local_head(block).expect("the definition is sealed");
    assert_ne!(definition, head);
    let edited = written(peer, block, &[(2, "TWO")]).await;
    let edit = match peer
        .seal(block, be_block_client::Write::written(edited, Some(head)))
        .await
        .expect("the edit is sealed")
    {
        be_block_client::Sealed::Sealed(edit) => edit,
        sealed => panic!("the edit changes the block: {sealed:?}"),
    };
    assert_eq!(
        peer.publish_sealed(block, edit)
            .await
            .expect("the edit is kept"),
        be_block_client::Saved::Kept(edit),
        "the edit waits behind the definition sealed before it"
    );
    (block, head, definition, edit)
}

/// The name and the given sections of the server's head of `block`, as the server has it.
async fn on_the_server(
    peer: &Peer<MemoryStore>,
    block: Uuid,
    sections: &[usize],
) -> (Option<String>, Vec<Option<Vec<u8>>>) {
    let head = peer
        .remote_head(block)
        .await
        .expect("the head is read")
        .expect("the block has a head");
    let name = peer
        .metadata_of(head)
        .await
        .expect("the metadata is read")
        .get(&be_block_content::NAME);
    let manifest = peer
        .load_commit(head)
        .await
        .expect("the head is read")
        .manifest;
    let stored = peer
        .load_manifest(&manifest)
        .await
        .expect("the head's content is read");
    let content = Sections::from_stored(&stored).expect("the sections decode");
    let sections = sections
        .iter()
        .map(|index| content.section(*index).map(<[u8]>::to_vec))
        .collect();
    (name, sections)
}
