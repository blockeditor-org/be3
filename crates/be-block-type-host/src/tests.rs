use std::sync::Arc;

use be_block::BlockContent;
use be_block_client::{ClientError, Credentials, Peer, PeerConfig};
use be_block_commit::CommitId;
use be_block_graph::BlockParent;
use be_block_store::{Manifest, MemoryStore};
use be_sdk_type_api::{Description, EditorId, Status};
use be_sdk_type_testing::probe::{self, Probe};
use be_sdk_type_testing::sections::{self, Sections};
use be_wasm_threads::Mutex;
use uuid::Uuid;

use crate::{Conflict, Host, Replicas, in_process, started};

mod a_block_whose_creation_waits_joins_once_it_is_sent_not_on_reconnecting;
mod a_journal_is_opened_on_the_commit_a_seal_was_made_on_only_for_this_devices_noted_seal;
mod a_merge_over_parts_reads_only_the_parts_each_side_changed;
mod a_modules_answer_is_seen_after_the_host_task_spent_its_budget;
mod a_relayed_message_sealed_with_a_key_another_search_awaits_waits_for_it;
mod a_stateless_request_reads_only_parts_of_the_commits_it_was_handed;
mod an_edit_from_an_editor_that_left_before_its_block_joined_is_kept;
mod search_text_of_a_type_that_loads_whole_is_narrowed_to_the_keys_asked_for;
mod search_text_reads_only_the_parts_asked_for_and_not_yet_indexed;
mod two_hosts_in_one_process_each_send_a_type_to_their_own_module;
mod what_a_wait_on_a_module_handles_is_taken_as_a_change;

/// A host that keeps nothing but which editors it was given frames for, for tests of stateless
/// requests.
#[derive(Default)]
struct Quiet {
    to_editors: Vec<EditorId>,
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
        shared.lock().unwrap().errors.push(error.to_string());
    }

    fn record_sync(shared: &Arc<Mutex<Self>>, error: ClientError) {
        Self::record(shared, error);
    }

    fn report(_: &Arc<Mutex<Self>>, _: Option<Uuid>, message: String) {
        eprintln!("{message}");
    }

    fn set_locked(_: &Arc<Mutex<Self>>, _: Uuid) {}

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

    fn to_editor(shared: &Arc<Mutex<Self>>, _: Uuid, editor: EditorId, _: Vec<u8>) -> bool {
        shared.lock().unwrap().to_editors.push(editor);
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
    with_devices(false, |peer, _, shared, replicas| {
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
    with_devices(true, |peer, another, shared, replicas| {
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
        let modules = started(
            vec![
                in_process::<sections::BlockTypes>("test.sections"),
                in_process::<probe::BlockTypes>("test.probe"),
                in_process::<be_type_text::BlockTypes>("be3.text"),
            ],
            be_block_client::Clock::real(),
        )
        .await;
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
