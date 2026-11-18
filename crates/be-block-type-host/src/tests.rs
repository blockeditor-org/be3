use std::sync::Arc;

use be_block_client::{ClientError, Credentials, Peer, PeerConfig};
use be_block_commit::CommitId;
use be_block_content::BlockContent;
use be_block_graph::BlockParent;
use be_block_store::{Manifest, MemoryStore};
use be_sdk_type_api::{Description, EditorId, Status};
use be_sdk_type_testing::probe::{self, Probe};
use be_sdk_type_testing::sections::{self, Sections};
use be_sdk_type_testing::ledger::Ledger;
use be_sdk_type_testing::tally::{Tally, TallyOp};
use std::sync::Mutex;
use uuid::Uuid;

use crate::{Conflict, Host, Replicas, in_process, started};

mod a_block_closed_while_its_rejoin_is_held_and_opened_online_again_holds_its_journaled_edits;
mod a_block_closed_while_its_rejoin_is_held_lets_go_of_the_join_and_closes;
mod a_block_reopened_while_its_rejoin_is_held_opens_again_holding_its_journaled_edits;
mod a_block_sent_while_its_replica_owes_a_seal_is_asked_for_it_once;
mod a_block_whose_creation_waits_joins_once_it_is_sent_not_on_reconnecting;
mod a_block_whose_kept_edits_stop_opening_as_its_replica_starts_opens_again_unlocked;
mod a_host_crash_with_a_held_drag_saves_nothing;
mod a_journal_is_opened_on_the_commit_a_seal_was_made_on_only_when_it_reaches_the_seal;
mod a_key_searched_for_in_vain_is_searched_for_again_after_reconnecting;
mod a_merge_over_parts_reads_only_the_parts_each_side_changed;
mod a_module_restart_mid_drag_keeps_the_drag;
mod a_modules_answer_is_seen_after_the_host_task_spent_its_budget;
mod a_rejoin_whose_unsealed_edits_no_seal_can_take_waits_without_asking_again;
mod a_relayed_message_sealed_with_a_key_another_search_awaits_waits_for_it;
mod a_reopened_replica_holds_an_editors_operations_less_those_replayed;
mod a_replica_opened_again_after_a_restart_is_told_it_and_its_edit_lands_once;
mod a_replica_opened_again_takes_the_servers_journal_and_this_device_keeps_it;
mod a_replica_reaches_no_part_of_a_block_it_was_not_handed;
mod a_replica_that_ended_uncounts_only_the_operations_it_never_took;
mod a_server_journal_off_the_heads_seals_is_found_without_reading_the_history;
mod a_stateless_request_reads_only_parts_of_the_commits_it_was_handed;
mod a_stream_of_relays_under_keys_that_cannot_be_got_costs_one_search_per_key;
mod an_edit_and_a_definition_kept_beside_another_devices_head_both_reach_the_server;
mod an_edit_carried_over_a_restart_keeps_the_identity_it_goes_again_under_across_a_crash;
mod an_edit_from_an_editor_that_left_before_its_block_joined_is_kept;
mod an_edit_made_offline_after_joining_a_session_another_device_numbered_is_taken;
mod an_edit_made_on_the_content_from_before_a_replica_joins_lands_after;
mod an_edit_numbered_offline_after_a_seal_survives_joining;
mod an_edit_numbered_offline_before_a_restart_survives_the_first_join_online;
mod an_edit_ordered_after_a_seal_is_given_once_to_a_restarted_replica;
mod an_edit_sealed_beside_a_commit_waiting_in_the_outbox_is_merged_with_it;
mod an_edit_sent_after_its_replica_made_a_clean_status_is_given_again_on_joining;
mod an_edit_submitted_before_its_edits_were_kept_by_an_idle_pass_is_known_after_a_crash;
mod an_editor_attached_when_its_replica_reopens_is_sent_the_content_again;
mod an_editor_of_a_replica_that_cannot_open_again_is_told_it_failed_and_the_block_released;
mod an_editor_reads_no_part_of_another_block_its_replica_was_not_handed;
mod an_editor_reattached_across_joining_is_sent_a_snapshot_holding_what_was_given_again;
mod news_of_a_block_whose_head_stayed_moves_no_replica;
mod search_text_of_a_type_that_loads_whole_is_narrowed_to_the_keys_asked_for;
mod search_text_reads_only_the_parts_asked_for_and_not_yet_indexed;
mod two_hosts_in_one_process_each_send_a_type_to_their_own_module;
mod only_the_types_own_app_writes_in_its_blocks_log;
mod what_a_module_appends_reaches_the_server_signed_by_the_host_as_its_apps;
mod what_a_module_says_during_a_wait_on_it_is_taken_without_another_wake;
mod what_a_replica_opened_offline_replayed_from_its_journal_survives_a_restart_of_its_module;
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
    /// The blocks whose replicas were let go, in order.
    released: Vec<Uuid>,
    /// How many times the replicas were counted.
    counted: usize,
    /// The blocks found locked, until keys arrive.
    locked: std::collections::HashSet<Uuid>,
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

    fn set_locked(shared: &Arc<Mutex<Self>>, block: Uuid, locked: bool) {
        let mut held = shared.lock().unwrap();
        match locked {
            true => held.locked.insert(block),
            false => held.locked.remove(&block),
        };
    }

    fn set_marked(_: &Arc<Mutex<Self>>, _: Uuid, _: bool) {}

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

    fn released(shared: &Arc<Mutex<Self>>, block: Uuid) {
        shared.lock().unwrap().released.push(block);
    }

    fn status(_: &Arc<Mutex<Self>>, _: Uuid, _: &Status) {}

    fn described(_: &Arc<Mutex<Self>>, _: Uuid, _: Vec<Uuid>, _: Description) {}

    fn conflict(_: &Arc<Mutex<Self>>, _: Uuid, _: Conflict) {}

    fn counted(
        shared: &Arc<Mutex<Self>>,
        _: Vec<crate::Owing>,
        _: Vec<crate::Owing>,
        _: Vec<crate::Owing>,
    ) {
        shared.lock().unwrap().counted += 1;
    }

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
    with_devices(
        true,
        be_block_client::Clock::real(),
        |peer, another, shared, replicas| {
            test(
                peer,
                another.expect("another device signed in"),
                shared,
                replicas,
            )
        },
    );
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
        listen(&peer);
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

thread_local! {
    /// What the connection the test listens to says: the order's entries and leases, which the
    /// worker's loop hands the replicas in be-host-stack.
    static EVENTS: std::cell::RefCell<Option<(
        Arc<be_block_client::Connection>,
        tokio::sync::broadcast::Receiver<be_block_client::Event>,
    )>> = const { std::cell::RefCell::new(None) };
}

/// Listens to `peer`'s connection, a new one after it reconnects.
fn listen(peer: &Peer<MemoryStore>) {
    let connection = peer.connection();
    EVENTS.with(|events| {
        let mut events = events.borrow_mut();
        if events
            .as_ref()
            .is_none_or(|(listened, _)| !Arc::ptr_eq(listened, &connection))
        {
            let receiver = connection.subscribe();
            *events = Some((connection, receiver));
        }
    });
}

/// Hands the replicas what the connection said since this was last asked.
fn hear(peer: &Arc<Peer<MemoryStore>>, shared: &Arc<Mutex<Quiet>>, replicas: &mut Replicas<Quiet>) {
    listen(peer);
    let heard: Vec<be_block_client::Event> = EVENTS.with(|events| {
        let mut events = events.borrow_mut();
        let Some((_, receiver)) = events.as_mut() else {
            return Vec::new();
        };
        std::iter::from_fn(|| receiver.try_recv().ok()).collect()
    });
    for event in heard {
        replicas.heard((peer, shared), &event);
    }
}

/// Waits for a module or the connection to say something, and hands the replicas all of it.
async fn step(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
) {
    hear(peer, shared, replicas);
    let listened = EVENTS.with(|events| events.borrow_mut().take());
    // A module's timer (a seal due once its edits go quiet) is fired as the worker's loop would.
    let wake_at = replicas.modules.wake_at();
    let clock = replicas.modules.clock().clone();
    let timer = async move {
        match wake_at {
            Some(at) => clock.sleep_until(at).await,
            None => std::future::pending().await,
        }
    };
    let listened = match listened {
        Some((connection, mut receiver)) => {
            let heard = tokio::select! {
                () = replicas.woken() => None,
                () = timer => None,
                heard = receiver.recv() => heard.ok(),
            };
            EVENTS.with(|events| *events.borrow_mut() = Some((connection, receiver)));
            heard
        }
        None => {
            tokio::select! {
                () = replicas.woken() => {}
                () = timer => {}
            }
            None
        }
    };
    replicas.modules.tick_due();
    if let Some(event) = listened {
        replicas.heard((peer, shared), &event);
    }
    hear(peer, shared, replicas);
    replicas.pump((peer, shared)).await;
}

/// Gives `mirror` what the replica sent `editor` since the frames were last taken, as the
/// editor would take it.
fn receive_frames<C: be_block_content::LiveEdit + Clone + Default>(
    shared: &Arc<Mutex<Quiet>>,
    editor: EditorId,
    mirror: &mut be_sdk_type_guest::editor::EditorMirror<C>,
) {
    let frames: Vec<Vec<u8>> = std::mem::take(&mut shared.lock().unwrap().frames)
        .into_iter()
        .filter(|(to, _)| *to == editor)
        .map(|(_, frame)| frame)
        .collect();
    for frame in frames {
        let message = frame
            .split_first_chunk::<4>()
            .filter(|(length, payload)| u32::from_be_bytes(**length) as usize == payload.len())
            .and_then(|(_, payload)| be_sdk_type_guest::editor::ToEditor::decode(payload))
            .or_else(|| be_sdk_type_guest::editor::ToEditor::decode(&frame));
        if let Some(message) = message {
            mirror.receive(message);
        }
    }
}

/// Attaches `editor` to the open `block` and waits until it is shown its content.
async fn attach_mirror<C: be_block_content::LiveEdit + Clone + Default>(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
    block: Uuid,
    editor: EditorId,
) -> be_sdk_type_guest::editor::EditorMirror<C> {
    let mut mirror = be_sdk_type_guest::editor::EditorMirror::<C>::default();
    replicas.attach(block, editor, true);
    replicas.relay_from_editor(
        block,
        editor,
        be_sdk_type_guest::editor::FromEditor::Resend.encode(),
        std::time::Instant::now(),
    );
    replicas.pump((peer, shared)).await;
    receive_frames(shared, editor, &mut mirror);
    while mirror.content().is_none() {
        replicas.woken().await;
        replicas.pump((peer, shared)).await;
        receive_frames(shared, editor, &mut mirror);
    }
    mirror
}

/// Types `text` at the start of what `mirror` shows, as its editor, numbered by the session
/// state it was shown, as an editor's edits are.
fn type_at_start(
    replicas: &mut Replicas<Quiet>,
    block: Uuid,
    editor: EditorId,
    mirror: &mut be_sdk_type_guest::editor::EditorMirror<be_type_text::TextContent>,
    text: &str,
) {
    let shown = mirror.content().expect("the editor was shown the text");
    let typed = be_type_text::TextBlock::insert(shown, 3, 0, text.as_bytes())
        .expect("there is somewhere to type");
    let message = mirror.operate(None, typed).expect("the editor has content");
    replicas.relay_from_editor(block, editor, message.encode(), std::time::Instant::now());
}

/// The modules of the test types and the text and counter types, on `clock`.
async fn test_modules(clock: be_block_client::Clock) -> crate::Modules {
    started(
        vec![
            in_process::<sections::BlockTypes>("test.sections"),
            in_process::<probe::BlockTypes>("test.probe"),
            in_process::<be_sdk_type_testing::tally::BlockTypes>("test.tally"),
            in_process::<be_sdk_type_testing::ledger::BlockTypes>("test.ledger"),
            in_process::<be_type_text::BlockTypes>("be3.text"),
            in_process::<be_type_example_counter::BlockTypes>("be3.counter"),
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
        .metadata_of(block, head)
        .await
        .expect("the metadata is read")
        .get(&be_block_content::NAME);
    let manifest = peer
        .load_reached(head)
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

// Of a `Tally` whose order restarts with an edit on its way.

/// Waits on the modules and what they answer, without hearing the connection: what the server
/// ordered meanwhile reaches the replicas only at the next `step`.
async fn pump_until(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
    what: &str,
    done: impl Fn(&Replicas<Quiet>) -> bool,
) {
    let waited = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        replicas.pump((peer, shared)).await;
        while !done(replicas) {
            replicas.woken().await;
            replicas.pump((peer, shared)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "never {what}");
}

async fn step_until_replicas(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
    what: &str,
    done: impl Fn(&Replicas<Quiet>, &Arc<Mutex<Quiet>>) -> bool,
) {
    let waited = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while !done(replicas, shared) {
            step(peer, shared, replicas).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "never {what}");
}

/// How many entries the server's order holds of `block`, as this device heard them.
fn heard_entries_of(peer: &Peer<MemoryStore>, block: Uuid) -> usize {
    peer.local_journal(block)
        .ok()
        .flatten()
        .map_or(0, |journal| journal.entries.len())
}

/// The last local edit the submissions `block`'s replica keeps hold.
fn last_kept(replicas: &Replicas<Quiet>, block: Uuid) -> u64 {
    replicas
        .submissions_of(block)
        .and_then(|(_, _, kept)| kept.into_iter().max())
        .unwrap_or_default()
}

fn operate(
    replicas: &mut Replicas<Quiet>,
    block: Uuid,
    editor: EditorId,
    mirror: &mut be_sdk_type_guest::editor::EditorMirror<Tally>,
    operation: TallyOp,
) {
    let message = mirror
        .operate(None, operation)
        .expect("the editor has content");
    replicas.relay_from_editor(block, editor, message.encode(), std::time::Instant::now());
}

/// Opens a new `Tally` and restarts its order (a seal of a session state past
/// `RESTART_STATE_BYTES`) while an edit adding 10 is on its way: the server numbers the edit
/// after the restart, every replica passes it over, and its replica submits it again carried
/// over. Returns once the host heard the restart, before it heard the carried edit ordered, with
/// the block and the number of the local edit it holds. Only what is written ahead of a
/// submission is kept (`hold_kept_edits`).
async fn carry_an_edit_over_a_restart(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
) -> (Uuid, u64) {
    let block = peer
        .create::<Tally>(BlockParent::Root)
        .await
        .expect("the tally is made");
    peer.save(block, &Tally::default(), None)
        .await
        .expect("the empty tally is published");
    replicas
        .open((peer, shared), block, Tally::CONTENT_TYPE, true)
        .await
        .expect("the block opens");
    while replicas.joining() {
        replicas.woken().await;
        replicas.pump((peer, shared)).await;
    }
    let editor = 7;
    let mut mirror = attach_mirror::<Tally>(peer, shared, replicas, block, editor).await;
    // Only what is written ahead of a submission is kept: no idle pass writes over it.
    replicas.hold_kept_edits = true;

    let grow = be_sdk_type_guest::RESTART_STATE_BYTES + 1;
    operate(replicas, block, editor, &mut mirror, TallyOp::Grow(grow));
    replicas.pump((peer, shared)).await;
    step_until_replicas(
        peer,
        shared,
        replicas,
        "had the growth ordered",
        |replicas, _| {
            heard_entries_of(peer, block) >= 1
                && replicas
                    .submissions_of(block)
                    .is_some_and(|(submitting, ..)| submitting == 0)
        },
    )
    .await;
    receive_frames(shared, editor, &mut mirror);

    // The seal restarts the order; the edit goes before the replica hears the restart.
    replicas.seal((peer, shared), block, false).await;
    pump_until(
        peer,
        shared,
        replicas,
        "submitted the restart",
        |replicas| {
            replicas
                .submissions_of(block)
                .is_some_and(|(submitting, ..)| submitting == 1)
        },
    )
    .await;
    let grown = last_kept(replicas, block);
    operate(replicas, block, editor, &mut mirror, TallyOp::Add(10));
    pump_until(peer, shared, replicas, "submitted the edit", |replicas| {
        last_kept(replicas, block) > grown
    })
    .await;
    let added = last_kept(replicas, block);
    step_until_replicas(
        peer,
        shared,
        replicas,
        "heard the restart",
        |replicas, _| {
            replicas
                .submissions_of(block)
                .is_some_and(|(_, restarts, _)| restarts >= 1)
        },
    )
    .await;
    let (_, _, kept) = replicas.submissions_of(block).expect("the block is open");
    assert_eq!(
        kept,
        [added],
        "the edit's submission carried over the restart is kept, and none from before it"
    );

    (block, added)
}

/// A text block typed in offline, "socks", whose replica, connected again, holds its rejoin for
/// the journaled edit no seal holds, as one whose seal backs off would
/// (`Replicas::rejoined_after_a_seal`): its closing seal takes it.
async fn text_whose_rejoin_is_held(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
) -> Uuid {
    use be_block_content::LiveEdit;
    use be_sdk_type_guest::editor::FromEditor;
    use be_type_text::{TextBlock, TextContent};

    let block = peer
        .create::<TextContent>(BlockParent::Root)
        .await
        .expect("the text is made");
    peer.save(block, &TextBlock::of(""), None)
        .await
        .expect("the empty text is published");
    let (connection, token) = (peer.connection(), peer.token());
    peer.detach();

    replicas
        .open((peer, shared), block, TextContent::CONTENT_TYPE, true)
        .await
        .expect("the block opens");
    replicas.pump((peer, shared)).await;
    let editor = 7;
    replicas.attach(block, editor, true);
    replicas.pump((peer, shared)).await;
    let content = TextContent::decode(
        &replicas
            .content((peer, shared), block)
            .await
            .expect("the text is open"),
    )
    .expect("the text decodes");
    let typed = TextBlock::insert(&content, 1, 0, b"socks").expect("there is somewhere to type");
    replicas.relay_from_editor(
        block,
        editor,
        FromEditor::Operate {
            gesture: None,
            operation: TextContent::encode_operation(&typed),
        }
        .encode(),
        std::time::Instant::now(),
    );
    replicas.pump((peer, shared)).await;
    replicas.modules.tick_due();
    replicas.pump((peer, shared)).await;
    let status = replicas.status(block).expect("the replica reported");
    assert!(
        status.journaled_taken > status.sealed_taken,
        "the edit is journaled, and no seal holds it"
    );

    peer.attach(connection, token)
        .await
        .expect("the device is connected again");
    replicas.rejoined_after_a_seal(block, status.journaled_taken, status.sealed_taken, false);
    replicas.pump((peer, shared)).await;
    assert!(replicas.joining(), "the rejoin is held");
    block
}

/// What the open text holds now, as its replica shows it.
async fn text_of(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
    block: Uuid,
) -> String {
    use be_block_content::BlockContent;
    use be_type_text::{TextBlock, TextContent};

    let bytes = replicas
        .content((peer, shared), block)
        .await
        .expect("the text is open");
    TextBlock::text(&TextContent::decode(&bytes).expect("the text decodes"))
}

/// Pumps until a replica opened since `openings` were noted has joined.
async fn opened_again(
    peer: &Arc<Peer<MemoryStore>>,
    shared: &Arc<Mutex<Quiet>>,
    replicas: &mut Replicas<Quiet>,
    block: Uuid,
    openings: usize,
) {
    while replicas.openings.len() == openings
        || replicas.joining()
        || replicas.status(block).is_none()
    {
        replicas.woken().await;
        replicas.pump((peer, shared)).await;
    }
    replicas.pump((peer, shared)).await;
}
