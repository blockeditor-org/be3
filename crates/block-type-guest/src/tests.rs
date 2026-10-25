use std::collections::{BTreeMap, BTreeSet, VecDeque};

use be_block::blob::{Blob, BlobKind};
use be_block::{BlockContent, ChildChange, LiveEdit, Root};
use be_commit::CommitId;
use be_model::{
    Anchor, BlockRef, Change as ModelChange, Count, Document, Edit, List, Model, ObjectId, Text,
};
use be_protocol::ClientId;
use be_session::{Lease, Resume};
use be_store::Hash;
use block_type_api::{
    Change, Description, EditorId, Endpoint, Opening, PublishOutcome, Recipients, SessionTag,
    Status,
    embed::{exports::replicas::Replica, host::Call},
    ops::{Connection, Incoming, OpId, Latest, Outcome, Reply, Side, guest::Executor},
};
use uuid::Uuid;

use crate::{
    Module, Registration,
    editor::{EditorMirror, FromEditor, Gesture, ToEditor},
};

mod a_conflicting_merge_keeps_the_unsealed_side_as_a_commit;
mod a_file_maps_to_a_block_and_back;
mod a_follower_edit_goes_beneath_a_peer_edit_the_owner_ordered_first;
mod a_follower_keeps_its_unconfirmed_edit_through_a_reload;
mod a_follower_replaces_the_content_and_the_owner_takes_it;
mod a_follower_takes_over_with_what_the_owner_had_not_sealed;
mod a_follower_that_claims_as_it_joins_waits_until_it_has_caught_up;
mod a_follower_that_missed_messages_catches_up_once_resynchronized;
mod a_follower_that_takes_over_after_a_seal_publishes_on_top_of_it;
mod a_follower_that_takes_over_saves_its_edit_too_large_to_relay;
mod a_follower_typing_while_the_owner_types_keeps_both_runs_whole;
mod a_followers_edit_too_large_to_relay_lands_once;
mod a_followers_edit_too_large_to_relay_lands_once_when_the_owner_leaves;
mod a_followers_kept_edits_reach_the_owner_that_takes_over;
mod a_followers_kept_edits_survive_the_owner_leaving_before_saving_them;
mod a_gesture_is_one_undo_step_and_quick_edits_absorb;
mod a_head_written_offline_merges_once_its_replica_opens;
mod a_late_joiner_places_an_insert_after_an_item_removed_before_it_joined;
mod a_late_joiner_types_where_it_sees_the_caret;
mod a_message_no_build_can_read_stops_a_follower_saving;
mod a_module_answers_stateless_requests;
mod a_peer_running_another_build_is_refused;
mod a_rejected_seal_merges_the_newer_head_and_publishes_again;
mod a_replaced_file_takes_none_of_the_old_files_header;
mod a_replacement_keeps_a_followers_unconfirmed_edits_in_history;
mod a_replacement_made_while_the_owner_is_gone_reaches_every_peer;
mod a_replacement_made_while_the_owner_is_gone_reaches_the_owner_that_takes_over;
mod an_abandoned_lease_is_claimed;
mod an_edit_too_large_to_relay_is_published_as_a_commit;
mod an_editor_joins_with_a_snapshot_and_follows_operations;
mod an_external_publish_merges_into_a_live_replica;
mod an_owner_whose_lease_lapsed_follows_whoever_claims_it;
mod child_changes_become_edits_of_the_parent;
mod closing_seals_and_releases;
mod concurrent_edits_converge;
mod edits_are_sealed_once_they_rest_for_the_seal_interval;
mod peer_messages_reach_one_peer_or_all_of_them;
mod presence_reaches_late_joiners_and_leaves_with_its_client;
mod typing_behind_an_unconfirmed_edit_goes_out_as_one_edit;
mod undo_reverts_only_this_devices_edit;

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
                Incoming::Called(called) => calls.extend(Call::accept_direct(&called)),
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
    session: be_protocol::SessionState,
) -> Opening {
    Opening {
        block: BLOCK,
        content_type: Uuid::nil(),
        organization: ORGANIZATION,
        client,
        tag: TAG,
        head,
        session,
    }
}

struct Device {
    client: ClientId,
    module: Option<Module>,
    executor: Option<Executor>,
    host: Connection,
    clock: Option<Latest<u64>>,
    replica: Option<Replica>,
    sleeping: Vec<(u64, Reply<()>)>,
    tag: SessionTag,
    status: Status,
    history: (bool, bool),
    described: Option<Description>,
    wake: Option<u64>,
    released: bool,
}

struct Sim<C: LiveEdit + Clone + Default> {
    now: u64,
    commits: BTreeMap<CommitId, Stored>,
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
    deaf: BTreeSet<usize>,
    largest_relay: usize,
    written: u64,
    make: fn() -> Module,
}

impl<C: LiveEdit + Clone + Default> Sim<C> {
    fn new(devices: usize, make: fn() -> Module) -> Self {
        Self {
            now: 1_000,
            commits: BTreeMap::new(),
            head: None,
            lease: Lease::new(),
            devices: (0..devices)
                .map(|index| Device {
                    client: index as ClientId + 1,
                    module: None,
                    executor: None,
                    host: Connection::new(Side::Host),
                    clock: None,
                    replica: None,
                    sleeping: Vec::new(),
                    tag: TAG,
                    status: Status::default(),
                    history: (false, false),
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
            deaf: BTreeSet::new(),
            largest_relay: 0,
            written: 0,
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
        self.lease.join(client, self.now as i64);
        self.to(device, |host| {
            block_type_api::embed::exports::replicas::start(host, vec![0; 32])
        });
        self.settle();
        let opening = Opening {
            content_type: C::CONTENT_TYPE,
            tag: self.devices[device].tag,
            ..opening(client, local, self.lease.state().clone())
        };
        let replica = self.to(device, |host| Replica::new(host, opening));
        self.devices[device].replica = Some(replica);
        if let Some(head) = self.head
            && local != self.head
        {
            self.with_replica(device, |replica, host| {
                replica.head_moved(host, head, false)
            });
        }
        self.lease_changed(Some(device));
        self.settle();
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
        self.with_replica(device, |replica, host| {
            replica.message(host, Endpoint::Editor(editor), message.encode())
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
            self.to(device, |host| clock.send(host, now));
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
        self.written += 1;
        let id = CommitId::from_hash(Hash::of_parts(&[&bytes, &self.written.to_be_bytes()]));
        self.commits.insert(id, Stored { parents, bytes });
        id
    }

    fn publish_elsewhere(&mut self, content: &C) -> CommitId {
        let head = self.write(content.encode(), self.head.into_iter().collect());
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
            self.with_replica(device, |replica, host| {
                replica.head_moved(host, head, false)
            });
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
                        reply.fail(host, block_type_api::ops::OpError::Canceled)
                    });
                }
            }
            Incoming::Called(called) => {
                if let Some(call) = Call::accept_direct(&called) {
                    self.act(device, call);
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
                let mut clock = Latest::new(stream);
                self.to(device, |host| clock.send(host, now));
                self.devices[device].clock = Some(clock);
            }
            Call::SleepUntil { reply, at } => {
                self.devices[device].wake = Some(at);
                self.devices[device].sleeping.push((at, reply));
            }
            Call::Send { to, bytes } => match to {
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
                        self.relay(to, client, bytes);
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
                            self.relay(to, client, bytes.clone());
                        }
                    }
                }
            },
            Call::Publish {
                reply,
                expected,
                merging,
                content,
            } => {
                if expected != self.head {
                    self.rejections += 1;
                    let head = self.head;
                    return self.to(device, |host| {
                        reply.send(host, &Ok(PublishOutcome::Rejected(head)))
                    });
                }
                if let Some(head) = expected
                    && merging.is_none()
                    && self.commits[&head].bytes == content.bytes
                {
                    return self.to(device, |host| {
                        reply.send(host, &Ok(PublishOutcome::Published(head)))
                    });
                }
                let parents = expected
                    .into_iter()
                    .chain(merging.map(|merging| merging.ours))
                    .collect();
                let head = self.write(content.bytes, parents);
                self.head = Some(head);
                self.publishes += 1;
                self.to(device, |host| {
                    reply.send(host, &Ok(PublishOutcome::Published(head)))
                });
                if !self.quiet {
                    self.moved(head, Some(device));
                }
            }
            Call::Stash {
                reply,
                parent,
                content,
            } => {
                let commit = self.write(content.bytes, parent.into_iter().collect());
                self.to(device, |host| reply.send(host, &Ok(commit)));
            }
            Call::Read { reply, commit, .. } => {
                let result = self
                    .commits
                    .get(&commit)
                    .map(|stored| stored.bytes.clone())
                    .ok_or_else(|| "no such commit".to_owned());
                self.to(device, |host| reply.send(host, &result));
            }
            Call::ResumeSession { reply, local } => {
                let resume = self.resume(local, self.head);
                self.to(device, |host| reply.send(host, &Ok(resume)));
            }
            Call::ClaimLease { generation } => {
                self.lease.claim(client, generation, self.now as i64);
                self.lease_changed(None);
            }
            Call::ReportClean {
                generation,
                clean_at,
            } => {
                self.lease
                    .heartbeat(client, generation, clean_at, self.now as i64);
            }
            Call::ReportStatus { status } => self.devices[device].status = status,
            Call::Described { description } => {
                self.devices[device].described = Some(description);
            }
            Call::History { can_undo, can_redo } => {
                self.devices[device].history = (can_undo, can_redo);
            }
            Call::Conflict {
                ours,
                theirs,
                count,
            } => self.conflicts.push((device, ours, theirs, count as usize)),
            Call::Error { message } => self.errors.push((device, message)),
        }
    }

    fn relay(&mut self, to: usize, from: ClientId, bytes: Vec<u8>) {
        self.largest_relay = self.largest_relay.max(bytes.len());
        if self.deaf.contains(&to) {
            return;
        }
        self.with_replica(to, |replica, host| {
            replica.message(host, Endpoint::Peer(from), bytes)
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
