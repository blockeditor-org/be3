use std::collections::{BTreeMap, BTreeSet, VecDeque};

use be_block_commit::CommitId;
use be_block_content::blob::{Blob, BlobKind};
use be_block_content::{BlockContent, ChildChange, LiveEdit, Root};
use be_block_model::{
    Anchor, BlockRef, Change as ModelChange, Count, Document, Edit, List, Model, ObjectId, Text,
};
use be_block_protocol::ClientId;
use be_block_protocol::{
    JoinedParticipant, Journal, JournalBase, MembershipChange, SessionState,
};
use be_block_session::{Membership, Resume, Sequencer};
use be_block_store::Hash;
use be_sdk_type_api::{
    Change, Description, EditorId, Endpoint, Opening, PublishOutcome, Recipients, SessionTag,
    Status, UndoState,
    embed::{exports::replicas::Replica, host::Call},
    ops::{Backlog, Connection, Incoming, OpId, Outcome, Outlets, Reply, Side, guest::Executor},
};
use uuid::Uuid;

use crate::{
    Module, Registration,
    editor::{EditorMirror, FromEditor, Gesture, ToEditor},
    wire::{Entry, Submitted},
};
use be_block_content::{Merge, PartReader};
use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

mod a_break_ends_a_burst_but_not_an_open_gesture;
mod a_burst_of_view_state_writes_is_journaled_as_one_put_each;
mod a_conflicting_merge_keeps_the_unsealed_side_as_a_commit;
mod a_documents_parts_cross_the_type_protocol_in_their_shape;
mod a_drain_of_an_earlier_seal_leaves_an_edit_kept_after_it_sending;
mod a_file_maps_to_a_block_and_back;
mod a_forged_arrival_is_replaced_by_the_orders;
mod a_gesture_is_one_undo_step_and_quick_edits_absorb;
mod a_head_written_offline_merges_once_its_replica_opens;
mod a_holder_demoted_while_its_put_is_pending_sends_the_edit_to_the_new_holder;
mod a_holder_follows_whoever_claims_the_lease;
mod a_holder_streaming_faster_than_a_seal_restarts_and_loses_nothing;
mod a_holder_whose_merge_cannot_read_its_parts_makes_no_due_seal_that_would_be_rejected;
mod a_holder_whose_merge_cannot_read_its_parts_owes_no_seal_until_it_can;
mod a_holder_whose_seal_a_peer_outpaces_restarts_and_loses_nothing;
mod a_holder_with_nothing_to_do_sets_no_timer;
mod a_holders_edit_too_large_to_relay_goes_by_its_part_and_is_committed_at_once;
mod a_holders_edit_too_large_to_relay_is_lacking_until_its_part_is_put_and_appended;
mod a_journal_another_build_wrote_is_dropped_and_the_block_marked;
mod a_late_joiner_places_an_insert_after_an_item_removed_before_it_joined;
mod a_late_joiner_types_where_it_sees_the_caret;
mod a_log_longer_than_a_page_is_folded_whole;
mod a_merge_by_parts_waits_for_the_parts_it_reads_and_never_resolves_without_them;
mod a_merge_that_cannot_read_its_parts_is_reported_once_and_tried_again_on_a_timer;
mod a_merge_that_failed_is_not_retried_by_a_replica_that_stopped_owning_the_session;
mod a_merge_that_waits_for_something_other_than_a_part_is_reported_not_waited_for;
mod a_merge_whose_state_is_too_large_to_go_inline_reloads_by_a_part;
mod a_module_answers_stateless_requests;
mod a_new_holder_recovers_edits_from_the_journal_of_one_that_crashed;
mod a_new_holder_replays_a_journaled_edit_by_its_part;
mod a_part_longer_than_any_edit_is_not_read;
mod a_peer_edit_numbered_first_goes_beneath_an_unconfirmed_edit;
mod a_peer_keeps_its_unconfirmed_edit_through_a_reload;
mod a_peer_reads_a_part_again_when_a_read_fails;
mod a_peer_replaces_the_content_and_the_holder_takes_it;
mod a_peer_running_another_build_is_refused;
mod a_peer_takes_the_lease_with_what_the_holder_had_not_sealed;
mod a_peer_that_takes_the_lease_after_a_seal_publishes_on_top_of_it;
mod a_peer_that_takes_the_lease_saves_its_edit_too_large_to_relay;
mod a_peer_typing_while_the_holder_types_keeps_both_runs_whole;
mod a_peers_edit_after_it_reopens_on_the_same_connection_lands;
mod a_peers_edit_too_large_to_relay_is_lacking_until_the_holder_accepts_it;
mod a_peers_edit_too_large_to_relay_lands_once;
mod a_peers_edit_too_large_to_relay_lands_once_when_the_holder_leaves;
mod a_peers_move_of_an_item_held_mid_drag_shows_beneath_the_held_move;
mod a_peers_paste_too_large_to_relay_lands_once_over_the_holders_unsealed_edits_and_undoes_alone;
mod a_peers_put_that_failed_is_tried_again;
mod a_refusal_that_leaves_the_commit_in_the_outbox_fails_the_edit_instead_of_settling_it;
mod a_rejected_seal_merges_the_newer_head_and_publishes_again;
mod a_reload_the_server_refuses_is_made_again_and_seals_go_on;
mod a_replaced_file_takes_none_of_the_old_files_header;
mod a_replacement_keeps_a_peers_unconfirmed_edits_in_history;
mod a_replacement_made_while_the_holder_is_gone_reaches_every_peer;
mod a_replacement_made_while_the_holder_is_gone_reaches_the_holder_that_takes_the_lease;
mod a_replacement_mid_drag_ends_it;
mod a_replacement_on_its_way_is_not_counted_as_held_by_the_head;
mod a_replica_that_loads_only_the_header_reads_the_payload_on_demand;
mod a_replica_whose_content_does_not_decode_tells_its_editors_it_failed;
mod a_restart_keeps_a_peers_pending_edit_and_the_peers_cursors;
mod a_restart_on_a_seal_the_order_moved_past_keeps_what_came_after;
mod a_restart_replayed_on_a_journal_from_an_earlier_seal_carries_alike;
mod a_restart_that_removes_the_dragged_item_ends_it;
mod a_restart_waits_for_the_holder_to_be_alone_below_the_ceiling;
mod a_seal_counted_in_another_run_of_the_order_is_merged_not_taken_as_news;
mod a_seal_mid_drag_leaves_the_drag_out_of_the_commit;
mod a_seal_rejected_after_edits_journaled_during_it_merges_and_publishes_them_once;
mod a_seal_that_failed_is_tried_again_on_a_timer_and_owes_nothing_meanwhile;
mod a_seal_whose_journal_did_not_reach_the_server_opens_as_the_seal;
mod a_small_edit_after_a_large_one_lands_in_order_when_the_holder_reads_late;
mod a_snapshot_counts_an_editors_refused_operation;
mod a_status_counts_the_local_edits_its_replica_took_since_it_opened;
mod a_typed_edit_during_a_held_drag_reaches_peers_at_once;
mod an_edit_a_peers_replacement_dropped_unsent_stays_dropped_after_a_crash;
mod an_edit_given_again_with_the_submission_that_held_it_lands_once_when_both_reach_the_server;
mod an_edit_held_behind_one_whose_order_came_during_a_seal_goes_once_the_seal_lands;
mod an_edit_is_lacking_until_the_server_holds_it;
mod an_edit_kept_across_a_stop_made_before_a_restart_since_is_told_not_sent;
mod an_edit_numbered_while_its_replica_was_deaf_lands_once_after_another_seals_it;
mod an_edit_numbered_while_its_replica_was_deaf_lands_once_after_it_reopens;
mod an_edit_refused_as_this_account_may_not_edit_fails_and_stops_showing;
mod an_edit_still_unsent_when_its_replicas_reload_went_out_lands_after_a_crash;
mod an_edit_whose_origin_aged_out_of_the_base_fails_as_possibly_sent;
mod an_editor_cancels_a_gesture_the_replica_ended_so_its_late_frames_never_count;
mod an_editor_is_not_told_again_a_snapshot_it_holds_when_its_replica_reopens;
mod an_editor_joins_with_a_snapshot_and_follows_operations;
mod an_editor_keeps_its_landed_edits_through_a_replica_reopened_under_it;
mod an_editor_reads_content_its_replica_could_not_open_as_failed;
mod an_editors_edit_made_before_two_restarts_it_was_not_shown_lands;
mod an_editors_log_read_is_answered_though_its_replica_reopens_before_the_answer;
mod an_entry_no_build_can_read_is_passed_over_by_every_replica;
mod an_entry_told_better_replaces_the_one_held_and_unknown_never_does;
mod an_external_publish_merges_into_a_live_replica;
mod an_hour_of_steady_edits_makes_about_twelve_commits;
mod an_operation_adds_a_part_by_its_hash;
mod an_own_edit_numbered_behind_what_the_replica_applied_is_an_error;
mod arrivals;
mod cancelling_one_of_two_drags_keeps_the_other;
mod child_changes_become_edits_of_the_parent;
mod closing_seals_and_releases;
mod concurrent_edits_converge;
mod edits_after_an_own_entry_the_journal_holds_are_sent_again_though_the_base_names_no_longer_their_origin;
mod edits_are_committed_once_they_rest_for_the_quiet_interval;
mod edits_given_again_with_a_submission_that_holds_other_edits_go_anew;
mod edits_given_again_with_the_submission_that_held_them_carry_their_announcements;
mod edits_on_their_way_when_another_head_lands_land_on_it;
mod edits_peer_ops_and_a_close_during_a_pending_merge_wait_behind_it_and_land_on_its_result;
mod every_replica_stamps_an_operation_with_when_the_server_took_it;
mod frames_a_drag_sends_after_an_undo_cancelled_it_write_nothing;
mod one_editors_two_drags_end_on_their_own;
mod peer_messages_reach_one_peer_or_all_of_them;
mod peers_keep_what_the_holder_typed_while_its_seal_was_out;
mod presence_reaches_late_joiners_and_leaves_with_its_client;
mod status_cuts_count_a_held_drag_once_it_commits;
mod tentative;
mod the_next_editor_holds_the_lease_once_its_holder_leaves;
mod two_drags_in_one_block_commit_as_one_step_when_the_second_ends;
mod two_editors_of_one_block_on_one_device_each_land_what_they_type_unheard;
mod two_editors_typing_at_once_keep_their_own_undo_steps;
mod two_edits_one_submission_held_go_again_as_it_once_both_are_given_again;
mod two_oversized_edits_in_a_row_land_in_order;
mod two_replicas_append_to_one_log_and_fold_it_in_one_order;
mod typing_behind_an_unconfirmed_edit_goes_out_as_one_edit;
mod typing_while_a_seal_waits_for_the_server_shows_at_once;
mod undo_mid_drag_cancels_it;
mod undo_reverts_only_this_devices_edit;
mod what_is_typed_while_a_seal_waits_on_the_server_is_journaled_at_once;
mod what_is_typed_while_a_seal_waits_survives_a_crash_before_the_seal_is_answered;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
struct Checklist {
    items: List<ChecklistItem>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
struct ChecklistItem {
    text: String,
    done: bool,
}

impl Checklist {
    fn add(text: &str) -> (ObjectId, Edit) {
        let item = ChecklistItem {
            text: text.to_owned(),
            done: false,
        };
        let (id, change) = Self::ITEMS.insert(ObjectId::ROOT, Anchor::End, &item);
        (id, change.into())
    }

    fn set_text(item: ObjectId, text: &str) -> Edit {
        ChecklistItem::TEXT.set(item, &text.to_owned()).into()
    }

    fn remove(item: ObjectId) -> Edit {
        ModelChange::remove(item).into()
    }
}

impl Root for Checklist {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6368_6563_6b6c_6973_742d_626c_6f63_6b31);
}

type ChecklistContent = Document<Checklist>;

struct Bytes;

impl BlobKind for Bytes {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6279_7465_732d_6669_7874_7572_6500_0001);

    type Header = String;

    fn name(header: &String) -> &str {
        header
    }
}

impl crate::BlobFile for Bytes {
    const ANY_FILE: bool = true;

    fn header(name: &str, _: &[u8]) -> String {
        name.to_owned()
    }

    fn media_type(_: &String) -> String {
        String::new()
    }

    fn claims(_: &str, _: &[u8]) -> bool {
        true
    }
}

type FileContent = Blob<Bytes>;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
struct Counter {
    count: Count,
}

impl Counter {
    fn add(by: i64) -> Edit {
        Self::COUNT.add(ObjectId::ROOT, by).into()
    }
}

impl Root for Counter {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x636f_756e_7465_722d_626c_6f63_6b2d_0001);
}

type CounterContent = Document<Counter>;

/// A counter whose every edit announces itself, as a chat's message does.
#[derive(Clone, Debug, Default, PartialEq)]
struct AnnouncedCounter(CounterContent);

impl BlockContent for AnnouncedCounter {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x616e_6e6f_756e_6365_642d_6669_7874_0001);

    fn encode(&self) -> Vec<u8> {
        self.0.encode()
    }

    fn decode(bytes: &[u8]) -> Result<Self, be_block_content::ContentError> {
        CounterContent::decode(bytes).map(Self)
    }
}

impl LiveEdit for AnnouncedCounter {
    type Op = Edit;

    fn apply(&mut self, operation: &Edit) {
        self.0.apply(operation);
    }

    fn announcements(&self, _: &Edit) -> Vec<be_block_content::Announcement> {
        vec![be_block_content::Announcement {
            key: "counted".to_owned(),
            text: "counted".to_owned(),
            several: "counted".to_owned(),
            mentions: Vec::new(),
        }]
    }

    fn absorb_operation(operation: &mut Edit, next: Edit) -> Option<Edit> {
        CounterContent::absorb_operation(operation, next)
    }

    fn carry_over(operation: &Edit, before: &Self, after: &Self, client: u64) -> Option<Edit> {
        CounterContent::carry_over(operation, &before.0, &after.0, client)
    }

    fn session_state(&self) -> Vec<u8> {
        LiveEdit::session_state(&self.0)
    }

    fn adopt_session_state(&mut self, state: &[u8]) -> Result<(), be_block_content::ContentError> {
        LiveEdit::adopt_session_state(&mut self.0, state)
    }
}

impl be_block_content::Merge for AnnouncedCounter {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> be_block_commit::MergeResult<Self> {
        match CounterContent::merge3(&base.0, &ours.0, &theirs.0) {
            be_block_commit::MergeResult::Clean(value) => {
                be_block_commit::MergeResult::Clean(Self(value))
            }
            be_block_commit::MergeResult::Conflicted { value, conflicts } => {
                be_block_commit::MergeResult::Conflicted {
                    value: Self(value),
                    conflicts,
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Prose {
    body: Text,
}

impl Root for Prose {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7072_6f73_652d_6669_7874_7572_6500_0001);
}

type ProseContent = Document<Prose>;

fn prose(content: &ProseContent) -> String {
    content
        .text(ObjectId::ROOT, Prose::BODY)
        .map_or_else(String::new, |body| {
            String::from_utf8_lossy(&body.items()).into_owned()
        })
}

fn prose_of(text: &str) -> ProseContent {
    Document::new(&Prose {
        body: Text::new(text),
    })
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Folder {
    entries: List<FolderEntry>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct FolderEntry {
    block: BlockRef,
}

impl Root for Folder {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x666f_6c64_6572_2d66_6978_7475_7265_0001);

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        let listed = |block: Uuid| {
            self.entries
                .iter()
                .filter(move |entry| entry.block.0 == block)
        };
        Some(match change {
            ChildChange::Add(block) if listed(block).next().is_some() => Edit::default(),
            ChildChange::Add(block) => Self::ENTRIES
                .insert(
                    ObjectId::ROOT,
                    Anchor::End,
                    &FolderEntry {
                        block: BlockRef(block),
                    },
                )
                .1
                .into(),
            ChildChange::Delete(block) => listed(block)
                .map(|entry| ModelChange::remove(entry.id))
                .collect(),
            ChildChange::Replace { old, new } => listed(old)
                .map(|entry| FolderEntry::BLOCK.set(entry.id, &BlockRef(new)))
                .collect(),
            ChildChange::Reorder { .. } => return None,
        })
    }
}

type FolderContent = Document<Folder>;

const BLOCK: Uuid = Uuid::from_u128(0xb10c);
const ORGANIZATION: Uuid = Uuid::from_u128(0x0e9);
const TAG: SessionTag = SessionTag([7; 32]);

fn checklist_module() -> Module {
    crate::Module::new(vec![
        Registration::of::<ChecklistContent>().undo::<ChecklistContent>(),
    ])
}

fn counter_module() -> Module {
    Module::new(vec![
        Registration::of::<CounterContent>().undo::<CounterContent>(),
    ])
}

/// A counter with a signed log, whose name is what its log says, in the log's order.
#[derive(Clone, Debug, Default, PartialEq)]
struct Logbook {
    counter: CounterContent,
    said: Vec<String>,
}

impl BlockContent for Logbook {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6c6f_6762_6f6f_6b2d_626c_6f63_6b2d_0001);

    fn encode(&self) -> Vec<u8> {
        self.counter.encode()
    }

    fn decode(bytes: &[u8]) -> Result<Self, be_block_content::ContentError> {
        Ok(Self {
            counter: CounterContent::decode(bytes)?,
            said: Vec::new(),
        })
    }

    fn name(&self) -> Option<String> {
        (!self.said.is_empty()).then(|| self.said.join(" "))
    }
}

impl LiveEdit for Logbook {
    type Op = Edit;

    fn apply(&mut self, operation: &Edit) {
        self.counter.apply(operation);
    }
}

impl be_block_content::Merge for Logbook {
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> be_block_commit::MergeResult<Self> {
        match CounterContent::merge3(&base.counter, &ours.counter, &theirs.counter) {
            be_block_commit::MergeResult::Clean(counter) => {
                be_block_commit::MergeResult::Clean(Self {
                    counter,
                    said: Vec::new(),
                })
            }
            be_block_commit::MergeResult::Conflicted { value, conflicts } => {
                be_block_commit::MergeResult::Conflicted {
                    value: Self {
                        counter: value,
                        said: Vec::new(),
                    },
                    conflicts,
                }
            }
        }
    }
}

impl be_block_content::LogFold for Logbook {
    fn fold(&mut self, item: &be_block_content::LogItem) {
        let said = item
            .payload
            .as_deref()
            .map_or("(redacted)".to_owned(), |payload| {
                String::from_utf8_lossy(payload).into_owned()
            });
        self.said.push(said);
    }
}

fn logbook_module() -> Module {
    Module::new(vec![Registration::of::<Logbook>().log::<Logbook>()])
}

fn announced_counter_module() -> Module {
    Module::new(vec![Registration::of::<AnnouncedCounter>()])
}

fn prose_module() -> Module {
    Module::new(vec![Registration::of::<ProseContent>()])
}

fn prose_with_undo_module() -> Module {
    Module::new(vec![
        Registration::of::<ProseContent>().undo::<ProseContent>(),
    ])
}

fn file_module() -> Module {
    Module::new(vec![Registration::of::<FileContent>()])
}

fn folder_module() -> Module {
    Module::new(vec![Registration::of::<FolderContent>()])
}

fn add(text: &str) -> Edit {
    Checklist::add(text).1
}

fn texts(content: &ChecklistContent) -> Vec<String> {
    content
        .root()
        .items
        .iter()
        .map(|item| item.text.clone())
        .collect()
}

fn count(content: &CounterContent) -> i64 {
    content.root().count.get()
}

struct Stored {
    parents: Vec<CommitId>,
    bytes: Vec<u8>,
    manifest: be_block_store::Manifest,
    /// How far the order reaches, and how many restarts it holds, where a replica sealed it.
    sealed_through: Option<(u64, u64)>,
    /// Whether the order it was sealed through was numbered on its device alone.
    sealed_offline: bool,
}

const SEALING: Uuid = Uuid::from_u128(0x5ea1);

fn vault() -> be_block_store::Vault<be_block_store::MemoryStore> {
    let keys = be_block_store::Keyring::new();
    keys.insert(
        SEALING,
        be_block_store::ContentKey::from_bytes([7; 32]),
        uuid::Uuid::nil(),
    );
    be_block_store::Vault::new(be_block_store::MemoryStore::new(), keys)
        .with_chunker(be_block_store::ChunkerConfig::SMALL)
}

struct Lone {
    executor: Executor,
    host: Connection,
}

impl Lone {
    fn new(module: &Module) -> Self {
        Self {
            executor: module.executor(),
            host: Connection::new(Side::Host),
        }
    }

    fn exchange(&mut self) -> (Vec<Call>, Vec<(OpId, Outcome)>) {
        let mut calls = Vec::new();
        let mut completed = Vec::new();
        let mut outbound = Vec::new();
        self.executor
            .step(self.host.take_outbound(), &mut outbound)
            .unwrap();
        for frame in outbound {
            self.host.receive(&frame).unwrap();
        }
        while let Some(event) = self.host.next_event() {
            match event {
                Incoming::Started(started) => calls.extend(Call::accept(&mut self.host, started)),
                Incoming::Completed { op, outcome } => completed.push((op, outcome)),
                Incoming::Yielded { .. }
                | Incoming::Canceled { .. }
                | Incoming::Credited { .. } => {}
            }
        }
        (calls, completed)
    }
}

fn opening(
    client: ClientId,
    head: Option<CommitId>,
    session: be_block_protocol::SessionState,
) -> Opening {
    Opening {
        block: BLOCK,
        content_type: Uuid::nil(),
        organization: ORGANIZATION,
        account: Uuid::nil(),
        client,
        tag: TAG,
        origin: client,
        taken_before: 0,
        doubtful_through: 0,
        kept_restart: 0,
        kept_through: 0,
        kept_submitted: 0,
        kept_submissions: Vec::new(),
        head,
        session,
        journal: None,
        lost_edits: false,
        journal_kept: false,
        commits_kept: be_sdk_type_api::KeptCommits::None,
        refused_commits: Vec::new(),
        server_head: None,
        sealed_head: None,
        epoch: 0,
    }
}

struct Device {
    client: ClientId,
    /// What its replica's submissions name it, kept across reopenings, new for each opening.
    origin: u64,
    /// How many local edits earlier openings of its replica took (`opening.taken-before`):
    /// the simulated host gives none of them again.
    taken_before: u64,
    /// Whether its account may edit the block; a viewer's is not, which the server holds it to.
    may_edit: bool,
    module: Option<Module>,
    executor: Option<Executor>,
    host: Connection,
    clock: Option<Outlets<()>>,
    replica: Option<Replica>,
    sleeping: Vec<(u64, Reply<()>)>,
    tag: SessionTag,
    status: Status,
    history: (bool, bool),
    undo: UndoState,
    described: Option<Description>,
    wake: Option<u64>,
    released: bool,
}

struct Sim<C: LiveEdit + Clone + Default> {
    now: u64,
    commits: BTreeMap<CommitId, Stored>,
    vault: be_block_store::Vault<be_block_store::MemoryStore>,
    head: Option<CommitId>,
    /// The session's membership, as the server writes it.
    lease: SessionState,
    devices: Vec<Device>,
    editors: BTreeMap<(usize, EditorId), EditorMirror<C>>,
    received: BTreeMap<(usize, EditorId), Vec<ToEditor>>,
    /// Editors not hearing yet: what reaches them waits here until `hear`, as messages still
    /// on their way would.
    unheard: BTreeMap<(usize, EditorId), Vec<ToEditor>>,
    /// What each editor sent its replica, in order.
    from_editors: BTreeMap<(usize, EditorId), Vec<FromEditor>>,
    queue: VecDeque<(usize, Vec<u8>)>,
    replies: BTreeMap<(usize, OpId), Outcome>,
    publishes: usize,
    rejections: usize,
    conflicts: Vec<(usize, CommitId, CommitId, usize)>,
    errors: Vec<(usize, String)>,
    quiet: bool,
    stalled: bool,
    /// Part reads wait (held) until `unstall`, so a merge by parts is pending.
    read_stalled: bool,
    /// How often each device's replica asked the host to resume its session.
    resumed: Vec<usize>,
    /// Submissions wait until `unstall`.
    stalled_submits: bool,
    /// The largest submission the server takes (`MAX_RELAY_BYTES` in be-server): a larger one
    /// fails.
    largest_submission: usize,
    /// What the next `open` is told the host kept across a stop: the last restart its replica
    /// had applied and the number of the last kept edit (`opening.kept-restart`,
    /// `opening.kept-through`).
    kept_opening: Option<(u64, u64)>,
    /// What the next `reopen` is told the host submitted for the device's origin
    /// (`opening.kept-submitted`).
    kept_submitted: u64,
    /// What the next `reopen` is told earlier openings submitted of the edits given again
    /// (`opening.kept-submissions`).
    kept_submissions: Vec<be_sdk_type_api::KeptSubmission>,
    /// Submissions are numbered as a device offline numbers them: in its own order, which the
    /// server lacks, answered as kept and heard by the device alone.
    offline_submits: bool,
    /// The server's order.
    order: Sequencer,
    /// The order a device keeps offline (`offline_submits`), from where the server's stood.
    offline_order: Option<Sequencer>,
    /// How many submissions it numbered.
    numbered: usize,
    /// Commits are published to the device's outbox alone, not to the server.
    kept_publishes: bool,
    held: Vec<(usize, Call)>,
    deaf: BTreeSet<usize>,
    largest_relay: usize,
    /// What each device sent its peers, in order: (device, bytes).
    sent: Vec<(usize, Vec<u8>)>,
    part_reads: usize,
    parts_put: usize,
    fail_puts: usize,
    fail_reads: usize,
    /// Publishes answered with an error, as a device's seal fails offline when its block's key
    /// is not on it; and how many were.
    fail_publishes: usize,
    failed_publishes: usize,
    largest_written: usize,
    written: u64,
    journal: Option<Journal>,
    /// A seal that moves the head leaves the journal where it was, as a server that lost the
    /// seal's rebase does: the journal stays on an earlier seal.
    lose_rebases: bool,
    journal_tag: SessionTag,
    /// The block's signed log, as the server numbers and stamps it: each device's client is its
    /// author, and every entry verified.
    log: Vec<be_block_content::LogItem>,
    make: fn() -> Module,
}

impl<C: LiveEdit + Clone + Default> Sim<C> {
    fn new(devices: usize, make: fn() -> Module) -> Self {
        Self {
            now: 1_000,
            commits: BTreeMap::new(),
            vault: vault(),
            head: None,
            lease: SessionState::default(),
            devices: (0..devices)
                .map(|index| Device {
                    client: index as ClientId + 1,
                    origin: 0,
                    taken_before: 0,
                    may_edit: true,
                    module: None,
                    executor: None,
                    host: Connection::new(Side::Host),
                    clock: None,
                    replica: None,
                    sleeping: Vec::new(),
                    tag: TAG,
                    status: Status::default(),
                    history: (false, false),
                    undo: UndoState {
                        can_undo: false,
                        can_redo: false,
                        restarted_at: None,
                        lost_edits: false,
                    },
                    described: None,
                    wake: None,
                    released: false,
                })
                .collect(),
            editors: BTreeMap::new(),
            received: BTreeMap::new(),
            unheard: BTreeMap::new(),
            from_editors: BTreeMap::new(),
            queue: VecDeque::new(),
            replies: BTreeMap::new(),
            publishes: 0,
            rejections: 0,
            conflicts: Vec::new(),
            errors: Vec::new(),
            quiet: false,
            stalled: false,
            read_stalled: false,
            resumed: vec![0; devices],
            stalled_submits: false,
            largest_submission: usize::MAX,
            kept_opening: None,
            kept_submitted: 0,
            kept_submissions: Vec::new(),
            offline_submits: false,
            order: Sequencer::starting(0),
            offline_order: None,
            numbered: 0,
            kept_publishes: false,
            held: Vec::new(),
            deaf: BTreeSet::new(),
            largest_relay: 0,
            sent: Vec::new(),
            part_reads: 0,
            parts_put: 0,
            fail_puts: 0,
            fail_reads: 0,
            fail_publishes: 0,
            failed_publishes: 0,
            largest_written: 0,
            written: 0,
            journal: None,
            lose_rebases: false,
            journal_tag: TAG,
            log: Vec::new(),
            make,
        }
    }

    fn open_devices(&self) -> Vec<usize> {
        (0..self.devices.len())
            .filter(|device| self.devices[*device].module.is_some())
            .collect()
    }

    fn device_of(&self, client: ClientId) -> Option<usize> {
        self.devices
            .iter()
            .position(|device| device.client == client && device.module.is_some())
    }

    fn to<R>(&mut self, device: usize, call: impl FnOnce(&mut Connection) -> R) -> R {
        let result = call(&mut self.devices[device].host);
        for frame in self.devices[device].host.take_outbound() {
            self.queue.push_back((device, frame));
        }
        result
    }

    fn with_replica<R>(
        &mut self,
        device: usize,
        call: impl FnOnce(Replica, &mut Connection) -> R,
    ) -> R {
        let replica = self.devices[device]
            .replica
            .expect("the device has the block open");
        self.to(device, |host| call(replica, host))
    }

    /// What a replica opening at `head` loads, as `be_block_type_host::journal_for` gives it:
    /// the journal on `head`, or, when `head` is a seal made on the journal's base that the
    /// journal reaches (`reaches_seal`), the journal on that base, which it loads first, then
    /// takes `head` as its sealed head.
    fn journal_on(
        &self,
        head: Option<CommitId>,
    ) -> (
        Option<Journal>,
        Option<CommitId>,
        Option<be_sdk_type_api::SealedHead>,
    ) {
        let journal = self.journal.clone();
        if journal
            .as_ref()
            .is_some_and(|journal| journal.base.commit == head)
        {
            return (journal, head, None);
        }
        let sealed = head.and_then(|commit| {
            let stored = self.commits.get(&commit)?;
            let parent = match stored.parents.as_slice() {
                [] => None,
                [parent] => Some(*parent),
                _ => return None,
            };
            Some((commit, parent, stored.sealed_through?))
        });
        match (journal, sealed) {
            (Some(journal), Some((commit, parent, (through, restarts))))
                if journal.base.commit == parent
                    && crate::wire::BaseState::decode(&journal.base.state)
                        .map_or(0, |state| state.restarts)
                        == restarts
                    && be_block_session::reaches_seal(
                        journal.base.sequence,
                        journal.entries.iter().map(|entry| entry.sequence),
                        through,
                    ) =>
            {
                (
                    Some(journal),
                    parent,
                    Some(be_sdk_type_api::SealedHead { commit, through }),
                )
            }
            // The journal is on an earlier commit: the replica opens on it, as the order goes
            // on from there, and hears of the head after.
            (Some(journal), _) => {
                let base = journal.base.commit;
                (Some(journal), base, None)
            }
            (None, _) => (None, head, None),
        }
    }

    fn open(&mut self, device: usize) {
        self.open_at(device, self.head);
    }

    fn open_at(&mut self, device: usize, local: Option<CommitId>) {
        let client = self.devices[device].client;
        let module = (self.make)();
        let held = &mut self.devices[device];
        held.executor = Some(module.executor());
        held.module = Some(module);
        held.host = Connection::new(Side::Host);
        held.clock = None;
        held.sleeping.clear();
        held.released = false;
        // As a host keeps one origin per device and block, opening again goes on numbering
        // after what the order holds of the device's edits.
        if held.origin == 0 {
            held.origin = (client << 32) + 1;
        } else {
            held.taken_before += held.status.journaled_taken;
        }
        let may_edit = self.devices[device].may_edit;
        let tag = Hash::from_bytes(self.devices[device].tag.0);
        self.lease.write(
            self.order.sequence,
            client,
            MembershipChange::Joined(JoinedParticipant { tag, may_edit }),
        );
        self.to(device, |host| {
            be_sdk_type_api::embed::exports::replicas::start(host, vec![0; 32])
        });
        self.settle();
        let (journal, local, sealed_head) = self.journal_on(local);
        let settles_on = sealed_head.as_ref().map(|sealed| sealed.commit);
        let foreign = self.journal_tag != self.devices[device].tag;
        let lost_edits = foreign
            && journal
                .as_ref()
                .is_some_and(|journal| !journal.entries.is_empty());
        let opening = Opening {
            content_type: C::CONTENT_TYPE,
            tag: self.devices[device].tag,
            origin: self.devices[device].origin,
            journal: journal.filter(|_| !foreign),
            lost_edits,
            sealed_head,
            kept_restart: self.kept_opening.map_or(0, |(restart, _)| restart),
            kept_through: self.kept_opening.take().map_or(0, |(_, through)| through),
            ..opening(client, local, self.lease.clone())
        };
        let replica = self.to(device, |host| Replica::new(host, opening));
        self.devices[device].replica = Some(replica);
        if let Some(head) = self.head
            && local != self.head
            && settles_on != self.head
        {
            self.with_replica(device, |replica, host| replica.head_moved(host, head));
        }
        self.lease_changed(Some(device));
        self.settle();
    }

    /// Starts the device's replica over on the head and the journal on it, as a host does after
    /// it reconnects, and attaches its editors again.
    fn reopen(&mut self, device: usize) {
        let client = self.devices[device].client;
        let (journal, head, sealed_head) = self.journal_on(self.head);
        let held = &mut self.devices[device];
        // The simulated host gives again none of what the order holds of its edits.
        held.taken_before += held.status.journaled_taken;
        let opening = Opening {
            content_type: C::CONTENT_TYPE,
            tag: self.devices[device].tag,
            origin: self.devices[device].origin,
            taken_before: self.devices[device].taken_before,
            kept_submitted: std::mem::take(&mut self.kept_submitted),
            kept_submissions: std::mem::take(&mut self.kept_submissions),
            journal,
            sealed_head,
            epoch: self.devices[device].status.epoch + 1,
            ..opening(client, head, self.lease.clone())
        };
        self.with_replica(device, |replica, host| replica.reopen(host, opening));
        let editors: Vec<EditorId> = self
            .editors
            .keys()
            .filter(|(on, _)| *on == device)
            .map(|(_, editor)| *editor)
            .collect();
        for editor in editors {
            // The journal holds what each editor was told landed, as the host would count it.
            let acknowledged = self.editors[&(device, editor)].acknowledged();
            self.with_replica(device, |replica, host| {
                replica.attach(host, editor, true, acknowledged);
                replica.resent(host, editor)
            });
        }
        self.settle();
    }

    fn follow_lacking(&mut self, device: usize, editor: EditorId) {
        self.send_from_editor(device, editor, FromEditor::FollowLacking);
        self.settle();
    }

    fn lacking(&self, device: usize, editor: EditorId) -> usize {
        self.editors[&(device, editor)].lacking().len()
    }

    fn attach(&mut self, device: usize, editor: EditorId, editable: bool) {
        self.editors
            .insert((device, editor), EditorMirror::default());
        self.with_replica(device, |replica, host| {
            replica.attach(host, editor, editable, 0);
            replica.resent(host, editor)
        });
        self.settle();
    }

    fn detach(&mut self, device: usize, editor: EditorId) {
        self.with_replica(device, |replica, host| replica.detach(host, editor));
    }

    /// An edit `editor` makes in its gesture 0 while that is open (`gesture`), or outside any.
    fn operate(&mut self, device: usize, editor: EditorId, operation: C::Op) {
        let open = self.editors[&(device, editor)].gesture_open(0);
        self.operate_in(device, editor, open.then_some(0), operation);
    }

    /// An edit `editor` makes in its gesture `gesture`, or outside any: whether its mirror
    /// sent it, which it does not in a gesture the replica ended.
    fn operate_in(
        &mut self,
        device: usize,
        editor: EditorId,
        gesture: Option<u64>,
        operation: C::Op,
    ) -> bool {
        let mirror = self
            .editors
            .get_mut(&(device, editor))
            .expect("the editor is attached");
        assert!(mirror.is_loaded(), "an editor operates once it has content");
        let Some(message) = mirror.operate(gesture, operation) else {
            return false;
        };
        self.send_from_editor(device, editor, message);
        true
    }

    /// Begins or ends `editor`'s gesture 0.
    fn gesture(&mut self, device: usize, editor: EditorId, gesture: Gesture) {
        self.gesture_in(device, editor, 0, gesture);
    }

    /// Begins or ends `editor`'s gesture `id`, as its mirror sends it: an end of one the
    /// replica ended is not sent.
    fn gesture_in(&mut self, device: usize, editor: EditorId, id: u64, gesture: Gesture) {
        let message = self
            .editors
            .get_mut(&(device, editor))
            .expect("the editor is attached")
            .gesture(id, gesture);
        if let Some(message) = message {
            self.send_from_editor(device, editor, message);
        }
    }

    fn break_step(&mut self, device: usize, editor: EditorId) {
        self.send_from_editor(device, editor, FromEditor::Break);
    }

    /// Holds what reaches `editor` until `hear`, as messages still on their way.
    fn deafen(&mut self, device: usize, editor: EditorId) {
        self.unheard.entry((device, editor)).or_default();
    }

    /// Gives `editor` what it was sent while deaf, and sends what its mirror answers.
    fn hear(&mut self, device: usize, editor: EditorId) {
        for message in self.unheard.remove(&(device, editor)).unwrap_or_default() {
            let answer = self
                .editors
                .get_mut(&(device, editor))
                .and_then(|mirror| mirror.receive(message));
            if let Some(answer) = answer {
                self.send_from_editor(device, editor, answer);
            }
        }
        self.settle();
    }

    /// What `editor` on `device` sent its replica, in order.
    fn sent_from(&self, device: usize, editor: EditorId) -> Vec<FromEditor> {
        self.from_editors
            .get(&(device, editor))
            .cloned()
            .unwrap_or_default()
    }

    fn send_from_editor(&mut self, device: usize, editor: EditorId, message: FromEditor) {
        self.from_editors
            .entry((device, editor))
            .or_default()
            .push(message.clone());
        let now = self.now;
        self.with_replica(device, |replica, host| {
            replica.message(host, Endpoint::Editor(editor), message.encode(), now)
        });
    }

    fn answered(&mut self, device: usize, op: OpId) -> Outcome {
        self.settle();
        self.replies
            .remove(&(device, op))
            .expect("every request is answered")
    }

    fn change(&mut self, device: usize, change: Change) -> bool {
        let ticket = self.with_replica(device, |replica, host| replica.change(host, change));
        let outcome = self.answered(device, ticket.op());
        ticket.decode(outcome).expect("a change is answered")
    }

    fn content(&mut self, device: usize) -> C {
        let ticket = self.with_replica(device, |replica, host| replica.content(host));
        let outcome = self.answered(device, ticket.op());
        let bytes = ticket
            .decode(outcome)
            .expect("content is answered")
            .expect("the replica is readable");
        C::decode(&bytes).unwrap()
    }

    fn mirror(&self, device: usize, editor: EditorId) -> &C {
        self.editors[&(device, editor)]
            .content()
            .expect("the editor was sent its content")
    }

    /// The entries of the block's log `editor` holds, in order.
    fn mirror_log(&self, device: usize, editor: EditorId) -> Vec<be_block_content::LogItem> {
        self.editors[&(device, editor)].log().cloned().collect()
    }

    fn take_received(&mut self, device: usize, editor: EditorId) -> Vec<ToEditor> {
        self.received.remove(&(device, editor)).unwrap_or_default()
    }

    fn head_content(&self) -> C {
        match self.head {
            Some(head) => C::decode(&self.commits[&head].bytes).unwrap(),
            None => C::default(),
        }
    }

    fn claim(&mut self, device: usize) {
        self.with_replica(device, |replica, host| replica.claim(host));
    }

    fn close(&mut self, device: usize) {
        self.with_replica(device, |replica, host| replica.resource_drop(host));
        self.settle();
    }

    fn crash(&mut self, device: usize) {
        self.devices[device].module = None;
        self.devices[device].executor = None;
        self.queue.retain(|(to, _)| *to != device);
        let client = self.devices[device].client;
        self.lease
            .write(self.order.sequence, client, MembershipChange::Left);
        self.lease_changed(Some(device));
        self.settle();
    }

    fn advance(&mut self, milliseconds: u64) {
        self.now += milliseconds;
        let now = self.now;
        for device in self.open_devices() {
            let Some(mut clock) = self.devices[device].clock.take() else {
                continue;
            };
            let _ = self.to(device, |host| clock.send(host, &now, |_| true));
            self.devices[device].clock = Some(clock);
            let held = std::mem::take(&mut self.devices[device].sleeping);
            let (due, waiting): (Vec<_>, Vec<_>) = held.into_iter().partition(|(at, _)| *at <= now);
            self.devices[device].wake = waiting.iter().map(|(at, _)| *at).min();
            self.devices[device].sleeping = waiting;
            for (_, reply) in due {
                self.to(device, |host| reply.send(host, &()));
            }
        }
        self.settle();
    }

    fn write(&mut self, bytes: Vec<u8>, parents: Vec<CommitId>) -> CommitId {
        self.write_parts(be_block_content::Part::from(bytes), parents)
    }

    fn write_parts(
        &mut self,
        parts: be_block_content::Part<'_>,
        parents: Vec<CommitId>,
    ) -> CommitId {
        let manifest = self.vault.write(SEALING, C::CONTENT_TYPE, parts).unwrap();
        self.commit(manifest, parents)
    }

    fn commit(&mut self, manifest: be_block_store::Manifest, parents: Vec<CommitId>) -> CommitId {
        self.written += 1;
        let bytes = self.vault.read(&manifest).unwrap();
        let id = CommitId::from_hash(Hash::of_parts(&[&bytes, &self.written.to_be_bytes()]));
        self.commits.insert(
            id,
            Stored {
                parents,
                bytes,
                manifest,
                sealed_through: None,
                sealed_offline: false,
            },
        );
        id
    }

    fn publish_elsewhere(&mut self, content: &C) -> CommitId {
        let head = self.write_parts(content.parts(), self.head.into_iter().collect());
        self.head = Some(head);
        if !self.quiet {
            self.moved(head, None);
        }
        self.settle();
        head
    }

    fn moved(&mut self, head: CommitId, publisher: Option<usize>) {
        for device in self.open_devices() {
            if Some(device) == publisher {
                continue;
            }
            self.with_replica(device, |replica, host| replica.head_moved(host, head));
        }
    }

    fn lease_changed(&mut self, except: Option<usize>) {
        let session = self.lease.clone();
        for device in self.open_devices() {
            if Some(device) == except {
                continue;
            }
            let session = session.clone();
            self.with_replica(device, |replica, host| replica.lease(host, session));
        }
    }

    /// The journal starts from `base`, which `device` sealed, as the server trims it; the
    /// other participants hear of a seal that names a commit (`replica.sealed`).
    fn trim(&mut self, device: usize, base: JournalBase, new_commit: bool) {
        let base_run = |base: &JournalBase| {
            crate::wire::BaseState::decode(&base.state).map_or(0, |state| state.restarts)
        };
        let sealed = base.commit.map(|commit| (commit, base.sequence));
        let run = base_run(&base);
        if !(self.lose_rebases && new_commit) {
            self.rebase(device, base);
        }
        // As the server, which tells only a seal that moved the head.
        let Some((commit, through)) = sealed.filter(|_| new_commit) else {
            return;
        };
        for to in self.open_devices() {
            if to != device && !self.deaf.contains(&to) {
                self.with_replica(to, |replica, host| {
                    replica.sealed(host, commit, through, run)
                });
            }
        }
    }

    /// The journal starts from `base`, which `device` sealed.
    fn rebase(&mut self, device: usize, base: JournalBase) {
        // Another build's entries go: the holder could not have applied them.
        let other = self.journal_tag != self.devices[device].tag;
        let entries: Vec<_> = self
            .journal
            .take()
            .map(|journal| journal.entries)
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| !other && entry.sequence > base.sequence)
            .collect();
        if other || (entries.is_empty() && base.sequence < self.order.sequence) {
            self.order = Sequencer::starting(base.sequence);
        }
        self.journal = Some(Journal {
            tag: Hash::from_bytes(self.devices[device].tag.0),
            base,
            entries,
            lost: false,
        });
        self.journal_tag = self.devices[device].tag;
    }

    fn settle(&mut self) {
        let mut handled = 0;
        while let Some((device, frame)) = self.queue.pop_front() {
            handled += 1;
            assert!(handled < 1_000_000, "the simulation never settled");
            let Some(executor) = self.devices[device].executor.as_ref() else {
                continue;
            };
            let mut outbound = Vec::new();
            executor.step(vec![frame], &mut outbound).unwrap();
            for frame in outbound {
                self.devices[device].host.receive(&frame).unwrap();
                while let Some(event) = self.devices[device].host.next_event() {
                    self.heard(device, event);
                }
            }
            if self.devices[device].released {
                self.devices[device].module = None;
                self.devices[device].executor = None;
                self.devices[device].replica = None;
                let client = self.devices[device].client;
                self.lease
                    .write(self.order.sequence, client, MembershipChange::Left);
                self.lease_changed(Some(device));
            }
        }
    }

    fn heard(&mut self, device: usize, event: Incoming) {
        match event {
            Incoming::Started(started) => {
                let call = self.to(device, |host| Call::accept(host, started));
                if let Some(call) = call {
                    self.act(device, call);
                }
            }
            Incoming::Completed { op, outcome } => {
                if self.devices[device].replica.map(Replica::op) == Some(op) {
                    self.devices[device].released = true;
                } else {
                    self.replies.insert((device, op), outcome);
                }
            }
            Incoming::Canceled { op } => {
                let held = std::mem::take(&mut self.devices[device].sleeping);
                let (canceled, waiting): (Vec<_>, Vec<_>) =
                    held.into_iter().partition(|(_, reply)| reply.op() == op);
                self.devices[device].sleeping = waiting;
                for (_, reply) in canceled {
                    self.to(device, |host| {
                        reply.fail(host, be_sdk_type_api::ops::OpError::Canceled)
                    });
                }
            }
            Incoming::Credited { op } => {
                if let Some(mut clock) = self.devices[device].clock.take() {
                    self.to(device, |host| clock.credited(host, op));
                    self.devices[device].clock = Some(clock);
                }
            }
            Incoming::Yielded { .. } => {}
        }
    }

    fn kept(&self, unsaved: impl Fn(&C) -> bool) -> bool {
        let head = self.head.expect("there is a head");
        self.ancestors(head).into_iter().skip(1).any(|commit| {
            C::decode(&self.commits[&commit].bytes).is_ok_and(|content| unsaved(&content))
        })
    }

    fn ancestors(&self, from: CommitId) -> Vec<CommitId> {
        let mut seen = BTreeSet::new();
        let mut order = Vec::new();
        let mut queue = VecDeque::from([from]);
        while let Some(commit) = queue.pop_front() {
            if !seen.insert(commit) {
                continue;
            }
            order.push(commit);
            queue.extend(self.commits[&commit].parents.iter().copied());
        }
        order
    }

    fn resume(&self, local: Option<CommitId>, remote: Option<CommitId>) -> Resume {
        match (local, remote) {
            (None, None) => Resume::Empty,
            (None, Some(remote)) => Resume::FastForward { to: remote },
            (Some(local), None) => Resume::Publish { from: local },
            (Some(local), Some(remote)) if local == remote => Resume::UpToDate,
            (Some(local), Some(remote)) => {
                if self.ancestors(remote).contains(&local) {
                    // As the host: a seal of this session made on the commit the replica is
                    // on is its news, not a head to merge.
                    let stored = &self.commits[&remote];
                    if stored.parents == [local]
                        && !stored.sealed_offline
                        && let Some((through, restarts)) = stored.sealed_through
                    {
                        return Resume::Sealed {
                            commit: remote,
                            through,
                            restarts,
                        };
                    }
                    return Resume::FastForward { to: remote };
                }
                let ours = self.ancestors(local);
                if ours.contains(&remote) {
                    return Resume::Publish { from: local };
                }
                Resume::Merge {
                    base: self
                        .ancestors(remote)
                        .into_iter()
                        .find(|commit| ours.contains(commit)),
                    ours: local,
                    theirs: remote,
                }
            }
        }
    }

    fn act(&mut self, device: usize, call: Call) {
        let client = self.devices[device].client;
        match call {
            Call::Clock { stream } => {
                let now = self.now;
                let mut clock = Outlets::default();
                clock.insert(stream, (), Backlog::Latest);
                let _ = self.to(device, |host| clock.send(host, &now, |_| true));
                self.devices[device].clock = Some(clock);
            }
            Call::InStepAt { reply, .. } => {
                self.to(device, |host| reply.send(host, &()));
            }
            Call::SleepUntil { reply, at } => {
                self.devices[device].sleeping.push((at, reply));
                self.devices[device].wake = self.devices[device]
                    .sleeping
                    .iter()
                    .map(|(at, _)| *at)
                    .min();
            }
            Call::Send { reply, to, bytes } => {
                self.to(device, |host| reply.send(host, &()));
                // As the server: what the sender may not send reaches no one.
                if !matches!(to, Recipients::One(Endpoint::Editor(_) | Endpoint::Host)) {
                    self.sent.push((device, bytes.clone()));
                }
                match to {
                    Recipients::One(Endpoint::Editor(editor)) => {
                        let message = ToEditor::decode(&bytes).expect("an editor message");
                        self.received
                            .entry((device, editor))
                            .or_default()
                            .push(message.clone());
                        if let Some(unheard) = self.unheard.get_mut(&(device, editor)) {
                            unheard.push(message);
                            return;
                        }
                        let Some(mirror) = self.editors.get_mut(&(device, editor)) else {
                            return;
                        };
                        if let Some(answer) = mirror.receive(message) {
                            self.send_from_editor(device, editor, answer);
                        }
                    }
                    Recipients::One(Endpoint::Peer(peer)) => {
                        if let Some(to) = self.device_of(peer) {
                            self.relay_as(to, client, bytes);
                        }
                    }
                    Recipients::One(Endpoint::Host) => {}
                    Recipients::Peers => {
                        let participants = self.lease.clients();
                        for peer in participants {
                            if peer == client {
                                continue;
                            }
                            if let Some(to) = self.device_of(peer) {
                                self.relay_as(to, client, bytes.clone());
                            }
                        }
                    }
                }
            }
            Call::Publish {
                reply,
                expected,
                merging,
                content,
                sealed_through,
                journal_state,
                restarts,
            } if !self.stalled => {
                if self.fail_publishes > 0 {
                    self.fail_publishes -= 1;
                    self.failed_publishes += 1;
                    return self.to(device, |host| {
                        reply.send(host, &Err("the connection is gone".to_owned()))
                    });
                }
                if expected != self.head {
                    self.rejections += 1;
                    let head = self.head;
                    return self.to(device, |host| {
                        reply.send(host, &Ok(PublishOutcome::Rejected(head)))
                    });
                }
                self.largest_written = self.largest_written.max(content.bytes.len());
                let parts = be_sdk_type_api::written_parts(&content.bytes, &content.parts);
                let manifest = self.vault.write(SEALING, C::CONTENT_TYPE, parts).unwrap();
                // As the server takes a seal's base as it wins the head, and as the host trims
                // with no new commit when there is nothing to commit.
                let base = |commit| {
                    sealed_through
                        .zip(journal_state.clone())
                        .map(|(sequence, state)| JournalBase {
                            commit: Some(commit),
                            sequence,
                            state,
                        })
                };
                if let Some(head) = expected
                    && merging.is_none()
                    && self.commits[&head].manifest == manifest
                {
                    if let Some(base) = base(head)
                        && self.lease.is_holder(client)
                    {
                        self.trim(device, base, false);
                    }
                    let kept = self.kept_publishes;
                    return self.to(device, |host| {
                        let outcome = match kept {
                            true => PublishOutcome::Kept(head),
                            false => PublishOutcome::Published(head),
                        };
                        reply.send(host, &Ok(outcome))
                    });
                }
                let parents = expected
                    .into_iter()
                    .chain(merging.map(|merging| merging.ours))
                    .collect();
                let head = self.commit(manifest, parents);
                if let Some(stored) = self.commits.get_mut(&head) {
                    stored.sealed_through = sealed_through.map(|through| (through, restarts));
                    stored.sealed_offline = self.offline_order.is_some();
                }
                self.head = Some(head);
                self.publishes += 1;
                if let Some(base) = base(head) {
                    self.trim(device, base, true);
                }
                let kept = self.kept_publishes;
                self.to(device, |host| {
                    let outcome = match kept {
                        true => PublishOutcome::Kept(head),
                        false => PublishOutcome::Published(head),
                    };
                    reply.send(host, &Ok(outcome))
                });
                if !self.quiet {
                    self.moved(head, Some(device));
                }
            }
            call @ Call::Publish { .. } => self.held.push((device, call)),
            Call::Stash {
                reply,
                parent,
                content,
            } => {
                let parts = be_sdk_type_api::written_parts(&content.bytes, &content.parts);
                let manifest = self.vault.write(SEALING, C::CONTENT_TYPE, parts).unwrap();
                let commit = self.commit(manifest, parent.into_iter().collect());
                self.to(device, |host| reply.send(host, &Ok(commit)));
            }
            Call::Load {
                reply,
                commit,
                first_part,
            } => {
                let result = match self.commits.get(&commit) {
                    Some(stored) => {
                        let loaded = match first_part {
                            true => self.vault.load_first(&stored.manifest),
                            false => self.vault.load(&stored.manifest),
                        };
                        loaded
                            .map(be_sdk_type_api::Loaded::of)
                            .map_err(|error| error.to_string())
                    }
                    None => Err("no such commit".to_owned()),
                };
                self.to(device, |host| reply.send(host, &result));
            }
            call @ Call::ReadPart { .. } if self.read_stalled => self.held.push((device, call)),
            Call::ReadPart { reply, part, range } => {
                self.part_reads += 1;
                if self.fail_reads > 0 {
                    self.fail_reads -= 1;
                    let result: Result<Vec<u8>, String> = Err("the server is unreachable".into());
                    return self.to(device, |host| reply.send(host, &result));
                }
                let result = self
                    .vault
                    .read_part(part, range.offset, range.length)
                    .map_err(|error| error.to_string());
                self.to(device, |host| reply.send(host, &result));
            }
            Call::Append { reply, bytes, .. } => {
                let seq = self.log.len() as u64;
                let item = be_block_content::LogItem {
                    seq,
                    id: Uuid::from_u64_pair(client, seq),
                    author: Uuid::from_u64_pair(0, client),
                    arrival: self.now as i64,
                    payload: Some(bytes),
                    signer: be_block_content::Signer::Verified,
                    imported: None,
                };
                let place = be_sdk_type_api::host::LogPlace {
                    seq,
                    arrival: item.arrival,
                };
                let id = item.id;
                self.to(device, |host| {
                    reply.send(
                        host,
                        &Ok::<_, be_sdk_type_api::host::NotAppended>(
                            be_sdk_type_api::host::Appended {
                                id,
                                place: Some(place),
                            },
                        ),
                    )
                });
                self.log_item(item);
            }
            Call::LetGoRefused { reply, .. } => self.to(device, |host| reply.send(host, &())),
            Call::ReadLog { reply, from, limit } => {
                let page = be_block_content::LogPage {
                    size: self.log.len() as u64,
                    items: self
                        .log
                        .iter()
                        .skip(from as usize)
                        .take(limit as usize)
                        .cloned()
                        .collect(),
                    kept: Vec::new(),
                    refused: Vec::new(),
                };
                self.to(device, |host| reply.send(host, &Ok(page)));
            }
            call @ Call::PutPart { .. } if self.stalled => self.held.push((device, call)),
            Call::PutPart { reply, bytes } => {
                self.parts_put += 1;
                if self.fail_puts > 0 {
                    self.fail_puts -= 1;
                    let result: Result<be_block_content::PartRef, String> =
                        Err("the server is unreachable".to_owned());
                    return self.to(device, |host| reply.send(host, &result));
                }
                let result = self
                    .vault
                    .write(
                        SEALING,
                        Uuid::nil(),
                        be_block_content::Part::from(bytes.as_slice()),
                    )
                    .map_err(|error| error.to_string())
                    .and_then(|manifest| manifest.root.ok_or_else(|| "no bytes".to_owned()));
                self.to(device, |host| reply.send(host, &result));
            }
            Call::ResumeSession { reply, local } => {
                self.resumed[device] += 1;
                let resume = self.resume(local, self.head);
                self.to(device, |host| reply.send(host, &Ok(resume)));
            }
            Call::Claim { reply } => {
                self.to(device, |host| reply.send(host, &()));
                if !self.devices[device].may_edit {
                    return;
                }
                self.lease
                    .write(self.order.sequence, client, MembershipChange::Claimed);
                self.lease_changed(None);
            }
            call @ Call::Submit { .. } if self.stalled_submits => self.held.push((device, call)),
            Call::Submit {
                reply, submission, ..
            } => {
                // As the server: a viewer is refused for good, and a build the session does not
                // speak is refused.
                if !self.devices[device].may_edit {
                    let refused = be_sdk_type_api::host::Submitted::Refused("a viewer".into());
                    return self.to(device, |host| reply.send(host, &refused));
                }
                if !self.lease.speaks(client) {
                    let refused = be_sdk_type_api::host::Submitted::Failed("refused".into());
                    return self.to(device, |host| reply.send(host, &refused));
                }
                if submission.payload.len() > self.largest_submission {
                    let refused = be_sdk_type_api::host::Submitted::Failed("too large".into());
                    return self.to(device, |host| reply.send(host, &refused));
                }
                if self.offline_submits {
                    let order = self.order;
                    let op = self.offline_order.get_or_insert(order).number(
                        be_block_session::Submission {
                            id: be_block_session::OpId {
                                origin: submission.origin,
                                counter: submission.counter,
                            },
                            base: submission.base,
                            barrier: submission.barrier,
                            payload: submission.payload,
                        },
                        None,
                    );
                    let entry = be_block_protocol::JournalEntry {
                        sequence: op.sequence,
                        origin: op.id.origin,
                        counter: op.id.counter,
                        base: op.base,
                        floor: op.floor,
                        received: op.received,
                        payload: op.payload,
                    };
                    self.with_replica(device, |replica, host| replica.sequenced(host, vec![entry]));
                    return self.to(device, |host| {
                        reply.send(host, &be_sdk_type_api::host::Submitted::Kept)
                    });
                }
                self.number(device, submission);
                self.to(device, |host| {
                    reply.send(host, &be_sdk_type_api::host::Submitted::Held)
                });
            }
            Call::RebaseJournal { reply, base, .. } => {
                self.to(device, |host| reply.send(host, &()));
                // As the server: a trim with no new commit is the holder's, on the head.
                if self.lease.is_holder(client) && base.commit == self.head {
                    self.trim(device, base, false);
                }
            }
            Call::ReportStatus { reply, status } => {
                self.to(device, |host| reply.send(host, &()));
                if let Some(undo) = &status.undo {
                    self.devices[device].history = (undo.can_undo, undo.can_redo);
                    self.devices[device].undo = undo.clone();
                }
                self.devices[device].status = status;
            }
            Call::Described { reply, description } => {
                self.to(device, |host| reply.send(host, &()));
                self.devices[device].described = Some(description);
            }
            Call::Conflict {
                reply,
                ours,
                theirs,
                count,
            } => {
                self.to(device, |host| reply.send(host, &()));
                self.conflicts.push((device, ours, theirs, count as usize));
            }
            Call::Error { reply, message } => {
                self.to(device, |host| reply.send(host, &()));
                self.errors.push((device, message));
            }
        }
    }

    /// As the server: numbers `submission` from `device` (unless the journal holds its id),
    /// keeps it in the journal and hands it to every participant, `device` too.
    /// Takes `item` into the block's log, as the server would, and tells every open device's
    /// replica.
    fn log_item(&mut self, item: be_block_content::LogItem) {
        self.log.push(item.clone());
        for device in self.open_devices() {
            self.with_replica(device, |replica, host| {
                replica.logged(host, vec![item.clone()])
            });
        }
    }

    fn number(&mut self, device: usize, submission: be_sdk_type_api::Submission) {
        self.largest_relay = self.largest_relay.max(submission.payload.len());
        let held = self.journal.as_ref().is_some_and(|journal| {
            journal.entries.iter().any(|entry| {
                entry.origin == submission.origin && entry.counter == submission.counter
            })
        });
        if held {
            return;
        }
        let op = self.order.number(
            be_block_session::Submission {
                id: be_block_session::OpId {
                    origin: submission.origin,
                    counter: submission.counter,
                },
                base: submission.base,
                barrier: submission.barrier,
                payload: submission.payload,
            },
            Some(self.now as i64),
        );
        let entry = be_block_protocol::JournalEntry {
            sequence: op.sequence,
            origin: op.id.origin,
            counter: op.id.counter,
            base: op.base,
            floor: op.floor,
            received: op.received,
            payload: op.payload,
        };
        self.numbered += 1;
        self.journal_tag = self.devices[device].tag;
        // A journal starts on the block's head as the server has it.
        let head = self.head;
        let tag = Hash::from_bytes(self.devices[device].tag.0);
        self.journal
            .get_or_insert_with(|| Journal {
                tag,
                base: JournalBase {
                    commit: head,
                    sequence: 0,
                    state: Vec::new(),
                },
                entries: Vec::new(),
                lost: false,
            })
            .entries
            .push(entry.clone());
        for peer in self.lease.clients() {
            if let Some(to) = self.device_of(peer)
                && !self.deaf.contains(&to)
            {
                let entry = entry.clone();
                self.with_replica(to, |replica, host| replica.sequenced(host, vec![entry]));
            }
        }
    }

    /// Numbers `payload` as a submission of `device`'s client that its replica never made, as
    /// a forger's or another build's would be, based on the order as it stands.
    fn submit_as(&mut self, device: usize, payload: Vec<u8>) {
        let submission = be_sdk_type_api::Submission {
            origin: u64::MAX - device as u64,
            counter: self.numbered as u64 + 1,
            base: self.order.sequence,
            barrier: false,
            payload,
        };
        self.number(device, submission);
        self.settle();
    }

    /// The operations the journal holds past its base, decoded.
    fn journal_operations(&self) -> Vec<C::Op> {
        let Some(journal) = &self.journal else {
            return Vec::new();
        };
        journal
            .entries
            .iter()
            .filter(|entry| entry.sequence > journal.base.sequence)
            .filter_map(|entry| match Submitted::decode(&entry.payload)?.entry {
                Entry::Operation(be_block_session::Payload::Inline(bytes)) => {
                    C::decode_operation(&bytes).ok()
                }
                _ => None,
            })
            .collect()
    }

    fn unstall(&mut self) {
        self.stalled = false;
        self.read_stalled = false;
        self.stalled_submits = false;
        for (device, call) in std::mem::take(&mut self.held) {
            self.act(device, call);
        }
        self.settle();
    }

    /// Hands `to` a message as relayed from `from`.
    fn relay_as(&mut self, to: usize, from: ClientId, bytes: Vec<u8>) {
        self.largest_relay = self.largest_relay.max(bytes.len());
        if self.deaf.contains(&to) {
            return;
        }
        let now = self.now;
        self.with_replica(to, |replica, host| {
            replica.message(host, Endpoint::Peer(from), bytes, now)
        });
    }
}

impl Sim<ProseContent> {
    /// The client `editor` on `device` makes what it inserts under: its own, as each editor's
    /// `LiveText` has (be-sdk-text's live_text.rs), never the replica's session client, so two
    /// editors of one block on one device never name what they insert alike. Unlike LiveText's,
    /// which are random, these are derived from the device and the editor, so tests repeat.
    fn editor_client(device: usize, editor: EditorId) -> u64 {
        1 << 63 | (device as u64) << 32 | editor
    }

    fn type_at(&mut self, device: usize, editor: EditorId, at: usize, typed: &str) {
        let client = Self::editor_client(device, editor);
        let op = self
            .mirror(device, editor)
            .text(ObjectId::ROOT, Prose::BODY)
            .and_then(|body| body.insert(client, at, typed.as_bytes().to_vec()))
            .expect("there is somewhere to type");
        self.operate(device, editor, Prose::BODY.edit(ObjectId::ROOT, op).into());
    }

    fn erase(&mut self, device: usize, editor: EditorId, range: std::ops::Range<usize>) {
        let op = self
            .mirror(device, editor)
            .text(ObjectId::ROOT, Prose::BODY)
            .and_then(|body| body.delete(range))
            .expect("there is something to erase");
        self.operate(device, editor, Prose::BODY.edit(ObjectId::ROOT, op).into());
    }

    fn typed(&self, device: usize, editor: EditorId) -> String {
        prose(self.mirror(device, editor))
    }
}

thread_local! {
    /// The part the merge below reads, and what it read.
    static PART: Cell<Option<be_block_content::PartRef>> = const { Cell::new(None) };
    static READ: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}
/// A merge by parts that reads a saved part before it merges.
fn reading_merge(
    base: ChecklistContent,
    ours: ChecklistContent,
    theirs: ChecklistContent,
    parts: PartReader,
) -> Pin<Box<dyn Future<Output = Result<be_block_commit::MergeResult<ChecklistContent>, String>>>> {
    Box::pin(async move {
        let part = PART.with(Cell::get).ok_or("no part")?;
        let bytes = parts.read_all(part).await?;
        READ.with(|read| *read.borrow_mut() = bytes);
        Ok(ChecklistContent::merge3(&base, &ours, &theirs))
    })
}
fn merging_module() -> Module {
    let mut registration = Registration::of::<ChecklistContent>().undo::<ChecklistContent>();
    registration.replica_merge = Some(Rc::new(reading_merge as crate::replica::PartsMerge<_>));
    Module::new(vec![registration])
}
fn diverged() -> (Sim<ChecklistContent>, CommitId, CommitId) {
    let mut sim = Sim::<ChecklistContent>::new(1, merging_module);
    let mut base = ChecklistContent::default();
    let (first, created) = Checklist::add("first");
    base.apply(&created);
    let (second, created) = Checklist::add("second");
    base.apply(&created);
    let shared = sim.publish_elsewhere(&base);
    let edit = |item, text: &str| {
        let mut edited = base.clone();
        edited.apply(&Checklist::set_text(item, text));
        edited
    };
    let theirs = sim.publish_elsewhere(&edit(second, "second, revised"));
    let ours = sim.write(edit(first, "first, expanded").encode(), vec![shared]);
    let part = sim
        .vault
        .write(
            SEALING,
            Uuid::nil(),
            be_block_content::Part::from(&b"a closed segment"[..]),
        )
        .unwrap()
        .root
        .unwrap();
    PART.with(|held| held.set(Some(part)));
    (sim, ours, theirs)
}
/// A live pair of replicas whose owner has an unsealed edit, and the part their merge reads.
fn live() -> Sim<ChecklistContent> {
    let mut sim = Sim::<ChecklistContent>::new(2, merging_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, add("unsealed"));
    sim.settle();
    let part = sim
        .vault
        .write(
            SEALING,
            Uuid::nil(),
            be_block_content::Part::from(&b"a closed segment"[..]),
        )
        .unwrap()
        .root
        .unwrap();
    PART.with(|held| held.set(Some(part)));
    sim
}
fn waiting_merge(
    _: ChecklistContent,
    _: ChecklistContent,
    _: ChecklistContent,
    _: PartReader,
) -> Pin<Box<dyn Future<Output = Result<be_block_commit::MergeResult<ChecklistContent>, String>>>> {
    Box::pin(std::future::pending())
}
fn waiting_module() -> Module {
    let mut registration = Registration::of::<ChecklistContent>().undo::<ChecklistContent>();
    registration.replica_merge = Some(Rc::new(waiting_merge as crate::replica::PartsMerge<_>));
    Module::new(vec![registration])
}
