use std::collections::{BTreeMap, BTreeSet, VecDeque};

use be_block::{BlockContent, ChildChange, Counter, CounterContent, LiveEdit, Root};
use be_block::blob::{Blob, BlobKind};
use be_commit::CommitId;
use be_model::{Anchor, BlockRef, Change as ModelChange, Document, Edit, List, Model, ObjectId};
use be_protocol::ClientId;
use be_session::{Lease, Resume};
use be_store::Hash;
use block_type_api::{
    Change, Command, Description, EditorId, Endpoint, Event, HeadEvent, Recipients, ReplicaId,
    Reply, Request, RequestId, SessionTag, Status,
};
use uuid::Uuid;

use crate::{
    Module, Registration,
    editor::{EditorMirror, FromEditor, Gesture, ToEditor},
};

mod a_conflicting_merge_keeps_the_unsealed_side_as_a_commit;
mod a_file_maps_to_a_block_and_back;
mod a_follower_edit_goes_beneath_a_peer_edit_the_owner_ordered_first;
mod a_follower_takes_over_with_what_the_owner_had_not_sealed;
mod a_gesture_is_one_undo_step_and_quick_edits_absorb;
mod a_module_answers_stateless_requests;
mod a_peer_running_another_build_is_refused;
mod a_rejected_seal_merges_the_newer_head_and_publishes_again;
mod an_abandoned_lease_is_claimed;
mod an_editor_joins_with_a_snapshot_and_follows_operations;
mod an_external_publish_merges_into_a_live_replica;
mod child_changes_become_edits_of_the_parent;
mod closing_seals_and_releases;
mod concurrent_edits_converge;
mod edits_are_sealed_once_they_rest_for_the_seal_interval;
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
    fn header(name: &str, _: &[u8]) -> String {
        name.to_owned()
    }

    fn claims(_: &str, _: &[u8]) -> bool {
        true
    }
}

type FileContent = Blob<Bytes>;

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
const REPLICA: ReplicaId = 1;
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
    content.root().value()
}

struct Stored {
    parents: Vec<CommitId>,
    bytes: Vec<u8>,
}

struct Device {
    client: ClientId,
    module: Option<Module>,
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
    queue: VecDeque<(usize, Event)>,
    replies: BTreeMap<RequestId, Reply>,
    requests: RequestId,
    publishes: usize,
    rejections: usize,
    conflicts: Vec<(usize, CommitId, CommitId, usize)>,
    errors: Vec<(usize, String)>,
    quiet: bool,
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
            requests: 0,
            publishes: 0,
            rejections: 0,
            conflicts: Vec::new(),
            errors: Vec::new(),
            quiet: false,
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

    fn open(&mut self, device: usize) {
        let client = self.devices[device].client;
        self.devices[device].module = Some((self.make)());
        self.devices[device].released = false;
        self.lease.join(client, self.now as i64);
        self.queue
            .push_back((device, Event::Tick { now: self.now }));
        self.queue.push_back((
            device,
            Event::Open {
                replica: REPLICA,
                block: BLOCK,
                content_type: C::CONTENT_TYPE,
                organization: ORGANIZATION,
                client,
                tag: self.devices[device].tag,
                head: self.head,
                session: self.lease.state().clone(),
            },
        ));
        self.lease_changed(Some(device));
        self.settle();
    }

    fn attach(&mut self, device: usize, editor: EditorId, editable: bool) {
        self.editors
            .insert((device, editor), EditorMirror::default());
        self.queue.push_back((
            device,
            Event::Attach {
                replica: REPLICA,
                editor,
                editable,
            },
        ));
        self.settle();
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
        self.queue.push_back((
            device,
            Event::Message {
                replica: REPLICA,
                from: Endpoint::Editor(editor),
                bytes: message.encode(),
            },
        ));
    }

    fn request(&mut self, device: usize, request: Request) -> Reply {
        self.requests += 1;
        let id = self.requests;
        self.queue
            .push_back((device, Event::Request { id, request }));
        self.settle();
        self.replies.remove(&id).expect("every request is answered")
    }

    fn change(&mut self, device: usize, change: Change) -> Reply {
        self.request(
            device,
            Request::Change {
                replica: REPLICA,
                change,
            },
        )
    }

    fn content(&mut self, device: usize) -> C {
        match self.request(device, Request::Content { replica: REPLICA }) {
            Reply::Content(bytes) => C::decode(&bytes).unwrap(),
            reply => panic!("asked for content, got {reply:?}"),
        }
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

    fn close(&mut self, device: usize) {
        self.queue
            .push_back((device, Event::Close { replica: REPLICA }));
        self.settle();
    }

    fn crash(&mut self, device: usize) {
        self.devices[device].module = None;
        self.queue.retain(|(to, _)| *to != device);
        let client = self.devices[device].client;
        self.lease.leave(client, self.now as i64);
        self.lease_changed(Some(device));
        self.settle();
    }

    fn advance(&mut self, milliseconds: u64) {
        self.now += milliseconds;
        for device in self.open_devices() {
            self.queue
                .push_back((device, Event::Tick { now: self.now }));
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
            self.queue.push_back((
                device,
                Event::Head {
                    replica: REPLICA,
                    event: HeadEvent::Moved {
                        head,
                        from_this_device: false,
                    },
                },
            ));
        }
    }

    fn lease_changed(&mut self, except: Option<usize>) {
        let session = self.lease.state().clone();
        for device in self.open_devices() {
            if Some(device) == except {
                continue;
            }
            self.queue.push_back((
                device,
                Event::Lease {
                    replica: REPLICA,
                    session: session.clone(),
                    resynchronize: false,
                },
            ));
        }
    }

    fn settle(&mut self) {
        let mut handled = 0;
        while let Some((device, event)) = self.queue.pop_front() {
            handled += 1;
            assert!(handled < 100_000, "the simulation never settled");
            let Some(module) = self.devices[device].module.as_mut() else {
                continue;
            };
            for command in module.handle(event) {
                self.act(device, command);
            }
            if self.devices[device].released {
                self.devices[device].module = None;
                let client = self.devices[device].client;
                self.lease.leave(client, self.now as i64);
                self.lease_changed(Some(device));
            }
        }
    }

    fn reply_to(&mut self, device: usize, event: HeadEvent) {
        self.queue.push_back((
            device,
            Event::Head {
                replica: REPLICA,
                event,
            },
        ));
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

    fn act(&mut self, device: usize, command: Command) {
        let client = self.devices[device].client;
        match command {
            Command::Hello { .. } => {}
            Command::Send { to, bytes, .. } => match to {
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
            Command::Publish {
                token,
                expected,
                merging,
                content,
                ..
            } => {
                if expected != self.head {
                    self.rejections += 1;
                    return self.reply_to(
                        device,
                        HeadEvent::Rejected {
                            token,
                            head: self.head,
                        },
                    );
                }
                if let Some(head) = expected
                    && merging.is_none()
                    && self.commits[&head].bytes == content.bytes
                {
                    return self.reply_to(device, HeadEvent::Published { token, head });
                }
                let parents = expected
                    .into_iter()
                    .chain(merging.map(|merging| merging.ours))
                    .collect();
                let head = self.write(content.bytes, parents);
                self.head = Some(head);
                self.publishes += 1;
                self.reply_to(device, HeadEvent::Published { token, head });
                if !self.quiet {
                    self.moved(head, Some(device));
                }
            }
            Command::Stash {
                token,
                parent,
                content,
                ..
            } => {
                let commit = self.write(content.bytes, parent.into_iter().collect());
                self.reply_to(device, HeadEvent::Stashed { token, commit });
            }
            Command::Read { token, commit, .. } => {
                let result = self
                    .commits
                    .get(&commit)
                    .map(|stored| stored.bytes.clone())
                    .ok_or_else(|| "no such commit".to_owned());
                self.queue.push_back((
                    device,
                    Event::Read {
                        replica: REPLICA,
                        token,
                        result,
                    },
                ));
            }
            Command::Resume { token, local, .. } => {
                let resume = self.resume(local, self.head);
                self.queue.push_back((
                    device,
                    Event::Resumed {
                        replica: REPLICA,
                        token,
                        result: Ok(resume),
                    },
                ));
            }
            Command::ClaimLease { generation, .. } => {
                self.lease.claim(client, generation, self.now as i64);
                self.lease_changed(None);
            }
            Command::ReportClean {
                generation,
                clean_at,
                ..
            } => {
                self.lease
                    .heartbeat(client, generation, clean_at, self.now as i64);
            }
            Command::Release { .. } => self.devices[device].released = true,
            Command::WakeAt { at } => self.devices[device].wake = Some(at),
            Command::Status { status, .. } => self.devices[device].status = status,
            Command::Described { description, .. } => {
                self.devices[device].described = Some(description);
            }
            Command::History {
                can_undo, can_redo, ..
            } => self.devices[device].history = (can_undo, can_redo),
            Command::Conflict {
                ours,
                theirs,
                count,
                ..
            } => self.conflicts.push((device, ours, theirs, count)),
            Command::Error { message, .. } => self.errors.push((device, message)),
            Command::Reply { id, reply } => {
                self.replies.insert(id, reply);
            }
        }
    }

    fn relay(&mut self, to: usize, from: ClientId, bytes: Vec<u8>) {
        self.queue.push_back((
            to,
            Event::Message {
                replica: REPLICA,
                from: Endpoint::Peer(from),
                bytes,
            },
        ));
    }
}
