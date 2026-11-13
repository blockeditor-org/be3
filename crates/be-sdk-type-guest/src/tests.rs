use std::collections::{BTreeMap, BTreeSet, VecDeque};

use be_block::blob::{Blob, BlobKind};
use be_block::{BlockContent, ChildChange, LiveEdit, Root};
use be_block_commit::CommitId;
use be_block_model::{
    Anchor, BlockRef, Change as ModelChange, Count, Document, Edit, List, Model, ObjectId, Text,
};
use be_block_protocol::ClientId;
use be_block_session::{Lease, Resume};
use be_block_store::Hash;
use be_sdk_type_api::{
    Change, Delivery, Description, EditorId, Endpoint, Journal, JournalBase, Opening,
    PublishOutcome, Recipients, RelayKind, SessionTag, Status, UndoState,
    embed::{exports::replicas::Replica, host::Call},
    ops::{Backlog, Connection, Incoming, OpId, Outcome, Outlets, Reply, Side, guest::Executor},
};
use uuid::Uuid;

use crate::{
    Module, Registration,
    editor::{EditorMirror, FromEditor, Gesture, ToEditor},
};
use be_block::{Merge, PartReader};
use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

mod a_break_ends_a_burst_but_not_an_open_gesture;
mod a_conflicting_merge_keeps_the_unsealed_side_as_a_commit;
mod a_documents_parts_cross_the_type_protocol_in_their_shape;
mod a_drain_of_an_earlier_seal_leaves_an_edit_kept_after_it_sending;
mod a_file_maps_to_a_block_and_back;
mod a_follower_edit_goes_beneath_a_peer_edit_the_owner_ordered_first;
mod a_follower_keeps_its_unconfirmed_edit_through_a_reload;
mod a_follower_obeys_only_three_resends_in_a_row;
mod a_follower_reads_a_part_again_when_a_read_fails;
mod a_follower_replaces_the_content_and_the_owner_takes_it;
mod a_follower_takes_over_with_what_the_owner_had_not_sealed;
mod a_follower_that_cannot_read_a_part_catches_up_from_the_head;
mod a_follower_that_claims_as_it_joins_waits_until_it_has_caught_up;
mod a_follower_that_gave_up_twice_across_a_promotion_still_catches_up;
mod a_follower_that_missed_messages_catches_up_once_resynchronized;
mod a_follower_that_takes_over_after_a_seal_publishes_on_top_of_it;
mod a_follower_that_takes_over_saves_its_edit_too_large_to_relay;
mod a_follower_typing_while_the_owner_types_keeps_both_runs_whole;
mod a_follower_whose_catchup_was_lost_asks_again;
mod a_followers_edit_too_large_to_relay_is_lacking_until_the_owner_accepts_it;
mod a_followers_edit_too_large_to_relay_lands_once;
mod a_followers_edit_too_large_to_relay_lands_once_when_the_owner_leaves;
mod a_followers_edits_stay_lacking_through_a_takeover_until_its_seal;
mod a_followers_kept_edits_reach_the_owner_that_takes_over;
mod a_followers_kept_edits_survive_the_owner_leaving_before_saving_them;
mod a_followers_paste_too_large_to_relay_lands_once_over_the_owners_unsealed_edits_and_undoes_alone;
mod a_followers_put_that_failed_is_tried_again;
mod a_followers_stash_kept_offline_by_the_new_owner_keeps_lacking_until_the_outbox_drains;
mod a_forged_arrival_is_never_sequenced;
mod a_gesture_is_one_undo_step_and_quick_edits_absorb;
mod a_head_written_offline_merges_once_its_replica_opens;
mod a_journal_another_build_wrote_is_dropped_and_the_block_marked;
mod a_late_joiner_places_an_insert_after_an_item_removed_before_it_joined;
mod a_late_joiner_types_where_it_sees_the_caret;
mod a_merge_by_parts_waits_for_the_parts_it_reads_and_never_resolves_without_them;
mod a_merge_that_cannot_read_its_parts_is_reported_once_and_tried_again_on_a_timer;
mod a_merge_that_failed_is_not_retried_by_a_replica_that_stopped_owning_the_session;
mod a_merge_that_waits_for_something_other_than_a_part_is_reported_not_waited_for;
mod a_message_no_build_can_read_stops_a_follower_saving;
mod a_module_answers_stateless_requests;
mod a_new_owner_recovers_edits_from_the_journal_of_one_that_crashed;
mod a_new_owner_replays_a_journaled_edit_by_its_part;
mod a_part_longer_than_any_edit_is_not_read;
mod a_peer_running_another_build_is_refused;
mod a_promoted_followers_handed_stash_stops_lacking_once_it_keeps_it_itself;
mod a_refusal_that_leaves_the_commit_in_the_outbox_fails_the_edit_instead_of_settling_it;
mod a_rejected_seal_merges_the_newer_head_and_publishes_again;
mod a_replaced_file_takes_none_of_the_old_files_header;
mod a_replacement_keeps_a_followers_unconfirmed_edits_in_history;
mod a_replacement_made_while_the_owner_is_gone_reaches_every_peer;
mod a_replacement_made_while_the_owner_is_gone_reaches_the_owner_that_takes_over;
mod a_replica_that_loads_only_the_header_reads_the_payload_on_demand;
mod a_restart_keeps_a_followers_pending_edit_and_the_peers_cursors;
mod a_restart_waits_for_the_owner_to_be_alone_below_the_ceiling;
mod a_seal_rejected_after_edits_journaled_during_it_merges_and_publishes_them_once;
mod a_seal_that_failed_is_tried_again_on_a_timer_and_owes_nothing_meanwhile;
mod a_seal_whose_journal_did_not_reach_the_server_opens_as_the_seal;
mod a_small_edit_after_a_large_one_lands_in_order_when_the_owner_reads_late;
mod a_snapshot_taken_before_the_commit_does_not_diverge_a_follower;
mod a_stamp_that_would_not_fit_inline_is_left_off;
mod a_submit_relayed_as_less_than_it_asks_is_not_applied;
mod a_viewer_cannot_make_a_follower_read_a_part;
mod an_abandoned_lease_is_claimed;
mod an_edit_is_lacking_until_the_server_holds_it;
mod an_editor_joins_with_a_snapshot_and_follows_operations;
mod an_external_publish_merges_into_a_live_replica;
mod an_hour_of_steady_edits_makes_about_twelve_commits;
mod an_operation_adds_a_part_by_its_hash;
mod an_owner_demoted_while_its_put_is_pending_sends_the_edit_to_the_new_owner;
mod an_owner_renews_its_lease_while_it_holds_the_session;
mod an_owner_that_cannot_read_a_followers_part_has_it_sent_again;
mod an_owner_that_loses_the_session_with_an_append_in_flight_fails_its_edit;
mod an_owner_whose_lease_lapsed_follows_whoever_claims_it;
mod an_owner_whose_merge_cannot_read_its_parts_makes_no_due_seal_that_would_be_rejected;
mod an_owner_whose_merge_cannot_read_its_parts_owes_no_seal_until_it_can;
mod an_owners_edit_too_large_to_relay_goes_by_its_part_and_is_committed_at_once;
mod an_owners_edit_too_large_to_relay_is_lacking_until_its_part_is_put_and_appended;
mod arrivals;
mod child_changes_become_edits_of_the_parent;
mod closing_seals_and_releases;
mod concurrent_edits_converge;
mod edits_are_committed_once_they_rest_for_the_quiet_interval;
mod edits_peer_ops_and_a_close_during_a_pending_merge_wait_behind_it_and_land_on_its_result;
mod followers_keep_what_the_owner_typed_while_its_seal_was_out;
mod messages_piled_behind_an_unreadable_part_make_a_follower_catch_up;
mod peer_messages_reach_one_peer_or_all_of_them;
mod presence_reaches_late_joiners_and_leaves_with_its_client;
mod the_owner_stamps_a_followers_operation_with_when_the_server_took_it;
mod two_editors_typing_at_once_keep_their_own_undo_steps;
mod two_oversized_edits_in_a_row_land_in_order;
mod two_quick_edits_folded_into_one_append_are_both_held_by_it;
mod typing_behind_an_unconfirmed_edit_goes_out_as_one_edit;
mod typing_while_a_seal_waits_for_the_server_shows_at_once;
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
    sealed_through: Option<(u64, u64)>,
}

const SEALING: Uuid = Uuid::from_u128(0x5ea1);

fn vault() -> be_block_store::Vault<be_block_store::MemoryStore> {
    let keys = be_block_store::Keyring::new();
    keys.insert(SEALING, be_block_store::ContentKey::from_bytes([7; 32]));
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
        head,
        session,
        journal: None,
        lost_edits: false,
        journal_kept: false,
        commits_kept: be_sdk_type_api::KeptCommits::None,
        refused_commits: Vec::new(),
        server_head: None,
        sealed_head: None,
    }
}

struct Device {
    client: ClientId,
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
    lease: Lease,
    devices: Vec<Device>,
    editors: BTreeMap<(usize, EditorId), EditorMirror<C>>,
    received: BTreeMap<(usize, EditorId), Vec<ToEditor>>,
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
    /// Appends are answered as kept on the device alone, and reach no one.
    unheld_appends: bool,
    /// Commits are published to the device's outbox alone, not to the server.
    kept_publishes: bool,
    held: Vec<(usize, Call)>,
    deaf: BTreeSet<usize>,
    largest_relay: usize,
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
    /// The generation each journal entry was appended in, by sequence, as the host's journal
    /// keeps it.
    journal_generations: BTreeMap<u64, u64>,
    /// The seal a device made of the journal and the last sequence it holds, as the host's
    /// journal notes it with the seal's commit, until the next rebase.
    sealing: Option<(usize, CommitId, u64)>,
    journal_tag: SessionTag,
    appended: usize,
    make: fn() -> Module,
}

impl<C: LiveEdit + Clone + Default> Sim<C> {
    fn new(devices: usize, make: fn() -> Module) -> Self {
        Self {
            now: 1_000,
            commits: BTreeMap::new(),
            vault: vault(),
            head: None,
            lease: Lease::new(),
            devices: (0..devices)
                .map(|index| Device {
                    client: index as ClientId + 1,
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
            unheld_appends: false,
            kept_publishes: false,
            held: Vec::new(),
            deaf: BTreeSet::new(),
            largest_relay: 0,
            part_reads: 0,
            parts_put: 0,
            fail_puts: 0,
            fail_reads: 0,
            fail_publishes: 0,
            failed_publishes: 0,
            largest_written: 0,
            written: 0,
            journal: None,
            journal_generations: BTreeMap::new(),
            sealing: None,
            journal_tag: TAG,
            appended: 0,
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

    /// What `device`'s replica opening at `head` loads, as `be_block_type_host::journal_for`
    /// gives it: the journal on `head`, or, when `head` is the seal `device` noted of the
    /// journal and the journal reaches it (`reaches_seal`), the journal on the commit the seal
    /// was made on, which it loads first, then takes `head` as its sealed head.
    fn journal_on(
        &self,
        device: usize,
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
        let noted = |commit: CommitId| {
            self.sealing
                .is_some_and(|(by, sealing, _)| by == device && sealing == commit)
        };
        let sealed = head.filter(|commit| noted(*commit)).and_then(|commit| {
            let stored = self.commits.get(&commit)?;
            let parent = match stored.parents.as_slice() {
                [] => None,
                [parent] => Some(*parent),
                _ => return None,
            };
            Some((commit, parent, stored.sealed_through?))
        });
        match (journal, sealed) {
            (Some(journal), Some((commit, parent, (generation, through))))
                if journal.base.commit == parent
                    && be_block_session::reaches_seal(
                        journal.base.sequence,
                        journal.entries.iter().map(|entry| {
                            let of = self.journal_generations.get(&entry.sequence);
                            (entry.sequence, of.copied().unwrap_or(u64::MAX))
                        }),
                        generation,
                        through,
                    ) =>
            {
                (
                    Some(journal),
                    parent,
                    Some(be_sdk_type_api::SealedHead { commit, through }),
                )
            }
            _ => (None, head, None),
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
        let may_edit = self.devices[device].may_edit;
        self.lease.join(client, may_edit, self.now as i64);
        self.to(device, |host| {
            be_sdk_type_api::embed::exports::replicas::start(host, vec![0; 32])
        });
        self.settle();
        let (journal, local, sealed_head) = self.journal_on(device, local);
        let settles_on = sealed_head.as_ref().map(|sealed| sealed.commit);
        let foreign = self.journal_tag != self.devices[device].tag;
        let lost_edits = foreign
            && journal
                .as_ref()
                .is_some_and(|journal| !journal.entries.is_empty());
        let opening = Opening {
            content_type: C::CONTENT_TYPE,
            tag: self.devices[device].tag,
            journal: journal.filter(|_| !foreign),
            lost_edits,
            sealed_head,
            ..opening(client, local, self.lease.state().clone())
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
        let journal = self
            .journal
            .clone()
            .filter(|journal| journal.base.commit == self.head);
        let opening = Opening {
            content_type: C::CONTENT_TYPE,
            tag: self.devices[device].tag,
            journal,
            ..opening(client, self.head, self.lease.state().clone())
        };
        self.with_replica(device, |replica, host| replica.reopen(host, opening));
        let editors: Vec<EditorId> = self
            .editors
            .keys()
            .filter(|(on, _)| *on == device)
            .map(|(_, editor)| *editor)
            .collect();
        for editor in editors {
            self.with_replica(device, |replica, host| replica.attach(host, editor, true));
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

    fn resynchronize(&mut self, device: usize) {
        let session = self.lease.state().clone();
        self.with_replica(device, |replica, host| replica.lease(host, session, true));
        self.settle();
    }

    fn attach(&mut self, device: usize, editor: EditorId, editable: bool) {
        self.editors
            .insert((device, editor), EditorMirror::default());
        self.with_replica(device, |replica, host| {
            replica.attach(host, editor, editable)
        });
        self.settle();
    }

    fn detach(&mut self, device: usize, editor: EditorId) {
        self.with_replica(device, |replica, host| replica.detach(host, editor));
    }

    fn operate(&mut self, device: usize, editor: EditorId, operation: C::Op) {
        let message = self
            .editors
            .get_mut(&(device, editor))
            .and_then(|mirror| mirror.operate(operation))
            .expect("an editor operates once it has content");
        self.send_from_editor(device, editor, message);
    }

    fn gesture(&mut self, device: usize, editor: EditorId, gesture: Gesture) {
        self.send_from_editor(device, editor, FromEditor::Gesture(gesture));
    }

    fn send_from_editor(&mut self, device: usize, editor: EditorId, message: FromEditor) {
        let now = self.now;
        self.with_replica(device, |replica, host| {
            replica.message(host, Endpoint::Editor(editor), message.encode(), now, None)
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
        self.lease.leave(client, self.now as i64);
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
            self.devices[device].sleeping = waiting;
            for (_, reply) in due {
                self.to(device, |host| reply.send(host, &()));
            }
        }
        self.settle();
    }

    fn write(&mut self, bytes: Vec<u8>, parents: Vec<CommitId>) -> CommitId {
        self.write_parts(be_block::Part::from(bytes), parents)
    }

    fn write_parts(&mut self, parts: be_block::Part<'_>, parents: Vec<CommitId>) -> CommitId {
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
        let session = self.lease.state().clone();
        for device in self.open_devices() {
            if Some(device) == except {
                continue;
            }
            let session = session.clone();
            self.with_replica(device, |replica, host| replica.lease(host, session, false));
        }
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
                self.lease.leave(client, self.now as i64);
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
                self.devices[device].wake = Some(at);
                self.devices[device].sleeping.push((at, reply));
            }
            Call::Send {
                reply,
                to,
                kind,
                bytes,
            } => {
                self.to(device, |host| reply.send(host, &()));
                // As the server: what the sender may not send reaches no one.
                let may_edit = self.devices[device].may_edit;
                let allowed = match kind {
                    RelayKind::Peer => true,
                    RelayKind::Edit => may_edit,
                };
                if !allowed && !matches!(to, Recipients::One(Endpoint::Editor(_))) {
                    return;
                }
                match to {
                    Recipients::One(Endpoint::Editor(editor)) => {
                        let message = ToEditor::decode(&bytes).expect("an editor message");
                        self.received
                            .entry((device, editor))
                            .or_default()
                            .push(message.clone());
                        let Some(mirror) = self.editors.get_mut(&(device, editor)) else {
                            return;
                        };
                        if let Some(answer) = mirror.receive(message) {
                            self.send_from_editor(device, editor, answer);
                        }
                    }
                    Recipients::One(Endpoint::Peer(peer)) => {
                        if let Some(to) = self.device_of(peer) {
                            self.relay_as(to, client, kind, bytes);
                        }
                    }
                    Recipients::One(Endpoint::Host) => {}
                    Recipients::Peers => {
                        let participants = self.lease.state().participants.clone();
                        for peer in participants {
                            if peer == client {
                                continue;
                            }
                            if let Some(to) = self.device_of(peer) {
                                self.relay_as(to, client, kind, bytes.clone());
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
                if let Some(head) = expected
                    && merging.is_none()
                    && self.commits[&head].manifest == manifest
                {
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
                    stored.sealed_through = sealed_through;
                }
                if let Some((_, through)) = sealed_through {
                    self.sealing = Some((device, head, through));
                }
                self.head = Some(head);
                self.publishes += 1;
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
                    .read_part(part.into(), range.offset, range.length)
                    .map_err(|error| error.to_string());
                self.to(device, |host| reply.send(host, &result));
            }
            call @ Call::PutPart { .. } if self.stalled => self.held.push((device, call)),
            Call::PutPart { reply, bytes } => {
                self.parts_put += 1;
                if self.fail_puts > 0 {
                    self.fail_puts -= 1;
                    let result: Result<be_sdk_type_api::PartRef, String> =
                        Err("the server is unreachable".to_owned());
                    return self.to(device, |host| reply.send(host, &result));
                }
                let result = self
                    .vault
                    .write(SEALING, Uuid::nil(), be_block::Part::from(bytes.as_slice()))
                    .map_err(|error| error.to_string())
                    .and_then(|manifest| manifest.root.ok_or_else(|| "no bytes".to_owned()))
                    .map(be_sdk_type_api::PartRef::from);
                self.to(device, |host| reply.send(host, &result));
            }
            Call::ResumeSession { reply, local } => {
                self.resumed[device] += 1;
                let resume = self.resume(local, self.head);
                self.to(device, |host| reply.send(host, &Ok(resume)));
            }
            Call::ClaimLease { reply, generation } => {
                self.to(device, |host| reply.send(host, &()));
                if !self.devices[device].may_edit {
                    return;
                }
                self.lease.claim(client, generation, self.now as i64);
                self.lease_changed(None);
            }
            Call::ReportClean {
                reply,
                generation,
                clean_at,
            } => {
                self.to(device, |host| reply.send(host, &()));
                self.lease
                    .heartbeat(client, generation, clean_at, self.now as i64);
            }
            Call::Append {
                reply,
                generation,
                entries,
                relay,
                ..
            } => {
                if self.unheld_appends {
                    return self.to(device, |host| reply.send(host, &false));
                }
                if !self.devices[device].may_edit {
                    return self.to(device, |host| reply.send(host, &false));
                }
                self.to(device, |host| reply.send(host, &true));
                let kind = RelayKind::Edit;
                if relay {
                    let participants = self.lease.state().participants.clone();
                    for entry in &entries {
                        for peer in &participants {
                            if *peer == client {
                                continue;
                            }
                            if let Some(to) = self.device_of(*peer) {
                                self.relay_as(to, client, kind, entry.payload.clone());
                            }
                        }
                    }
                }
                let state = self.lease.state();
                if !state.is_owner(client) || state.generation != generation {
                    return;
                }
                self.appended += 1;
                if self.journal_tag != self.devices[device].tag {
                    return;
                }
                let journal = self.journal.get_or_insert_with(|| Journal {
                    base: JournalBase {
                        commit: None,
                        sequence: 0,
                        state: Vec::new(),
                    },
                    entries: Vec::new(),
                });
                for entry in entries {
                    let last = journal
                        .entries
                        .last()
                        .map_or(journal.base.sequence, |entry| entry.sequence);
                    if entry.sequence > last {
                        self.journal_generations.insert(entry.sequence, generation);
                        journal.entries.push(entry);
                    }
                }
            }
            Call::RebaseJournal {
                reply,
                generation,
                base,
                entries,
            } => {
                self.to(device, |host| reply.send(host, &()));
                let state = self.lease.state();
                if !state.is_owner(client) || state.generation != generation {
                    return;
                }
                let entries: Vec<_> = entries
                    .into_iter()
                    .filter(|entry| entry.sequence > base.sequence)
                    .collect();
                self.journal_generations = entries
                    .iter()
                    .map(|entry| (entry.sequence, generation))
                    .collect();
                self.sealing = None;
                self.journal = Some(Journal { base, entries });
                self.journal_tag = self.devices[device].tag;
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
            Call::Announce { reply, .. } => {
                self.to(device, |host| reply.send(host, &()));
            }
        }
    }

    fn unstall(&mut self) {
        self.stalled = false;
        self.read_stalled = false;
        for (device, call) in std::mem::take(&mut self.held) {
            self.act(device, call);
        }
        self.settle();
    }

    /// Hands `to` a message as relayed from `from` by a server that checked it as an editor's.
    fn relay(&mut self, to: usize, from: ClientId, bytes: Vec<u8>) {
        self.relay_as(to, from, RelayKind::Edit, bytes);
    }

    /// Hands `to` a message as relayed from `from` by a server that checked it as `kind`.
    fn relay_as(&mut self, to: usize, from: ClientId, kind: RelayKind, bytes: Vec<u8>) {
        self.largest_relay = self.largest_relay.max(bytes.len());
        if self.deaf.contains(&to) {
            return;
        }
        let now = self.now;
        let delivery = Some(Delivery {
            kind,
            received: now as i64,
        });
        self.with_replica(to, |replica, host| {
            replica.message(host, Endpoint::Peer(from), bytes, now, delivery)
        });
    }
}

impl Sim<ProseContent> {
    fn type_at(&mut self, device: usize, editor: EditorId, at: usize, typed: &str) {
        let client = self.devices[device].client;
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
    static PART: Cell<Option<be_block::PartRef>> = const { Cell::new(None) };
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
            be_block::Part::from(&b"a closed segment"[..]),
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
            be_block::Part::from(&b"a closed segment"[..]),
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
