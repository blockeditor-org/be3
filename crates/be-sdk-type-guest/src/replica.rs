use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use be_block::{Landed, LiveEdit, Merge, Settling};
use be_block_commit::{CommitId, MergeResult};
use be_block_protocol::{ClientId, SessionState};
use be_block_session::{
    Follower, Inherited, OpId, Payload, Resume, Sequencer, SessionMessage, SessionOp,
};
use be_block_store::{Hash, PartRef};
use be_sdk_type_api::{
    Change, EditorId, Endpoint, FileChange, Journal, JournalBase, JournalEntry, Merging,
    Delivery, Recipients, RelayKind, ReplicaId, SessionTag, Status, UndoState, Written,
};
use uuid::Uuid;

use crate::lacking::{Carried, Lacking, Until};
use crate::{
    FromFileFn, Recorder,
    editor::{EditorOperation, FromEditor, Gesture, PeerMessage, PeerPresence, Snapshot, ToEditor},
    effect::{Answer, Effect, HeadEvent, RequestId, Token},
    wire::Wire,
};

/// An owner commits once its session has gone this long with no edits: a natural pause,
/// which is the state retention prefers to keep.
pub const QUIET_INTERVAL: u64 = 30_000;

/// How long a follower waits before it puts again the part of an edit whose put failed.
const PUT_RETRY_INTERVAL: u64 = 5_000;

/// The longest a follower waits between puts of an edit's part, which double from
/// `PUT_RETRY_INTERVAL`.
const PUT_RETRY_CEILING: u64 = 60_000;

/// How many times a part is read before it counts as unavailable, and how long the first
/// wait is (it doubles each time).
const READ_ATTEMPTS: u32 = 4;

/// An owner reads a follower's part this many times, the waits growing to a minute, before
/// it gives up and tells the follower to submit again: it holds every message behind the read,
/// so it is slower to give up than a follower.
const OWNER_READ_ATTEMPTS: u32 = 10;

/// How many times a follower submits again at the owner's word before it leaves its edit
/// pending and reports it.
const MOST_RESENDS: u32 = 3;
const READ_RETRY_CEILING: u64 = 60_000;
const READ_RETRY_INTERVAL: u64 = 1_000;

/// The longest part a peer may name, and the most messages kept waiting behind a read. A
/// message over either is not waited for: the replica asks the owner to catch it up instead.
const LARGEST_PART: u64 = 64 * LARGEST_RELAYED_OPERATION as u64;
const MOST_HELD_JOBS: usize = 10_000;

/// An owner commits at most this long after the first edit since its last commit, while edits
/// go on, which bounds what a restore point misses and what a reader replays.
pub const COMMIT_INTERVAL: u64 = 5 * 60_000;

/// An owner commits once what it appended to the journal since its last commit passes this,
/// so replaying a journal stays well under a second.
pub const JOURNAL_BYTES: usize = 1024 * 1024;

/// An owner alone in its session appends what it sequenced this often, folded, so a crash
/// loses at most this much; with others there it appends each operation as it relays it.
pub const APPEND_INTERVAL: u64 = 250;

/// An owner restarts its session at a commit once the session state passes this and outgrows
/// the visible content: tombstones and positions kept for concurrent edits, which a reload from
/// the commit drops. It waits for a commit it makes alone, so nobody's edits need carrying over,
/// until the state passes `RESTART_CEILING` times this.
pub const RESTART_STATE_BYTES: usize = 1024 * 1024;

/// How far past `RESTART_STATE_BYTES` session state grows before an owner with others in the
/// session restarts anyway.
pub const RESTART_CEILING: usize = 4;

pub const HEARTBEAT_INTERVAL: u64 = be_block_session::LEASE_MILLISECONDS as u64 / 2;

const JOURNAL_LIMIT: usize = 1024;
/// An operation whose bytes pass this goes by the part they were put as (`Payload::Part`).
const LARGEST_RELAYED_OPERATION: usize = 1024 * 1024;
const REPLACE_ATTEMPTS: u8 = 4;
/// How many commits a follower kept the owner remembers having saved.
const SAVED_KEPT: usize = 64;
const DIVERGED: &str = "an edit from a peer running a different version";

pub(crate) struct Opened {
    pub(crate) replica: ReplicaId,
    pub(crate) content_type: Uuid,
    pub(crate) organization: Uuid,
    pub(crate) client: ClientId,
    pub(crate) tag: SessionTag,
    pub(crate) head: Option<CommitId>,
    pub(crate) session: SessionState,
    pub(crate) journal: Option<Journal>,
    pub(crate) sealed_head: Option<be_sdk_type_api::SealedHead>,
    pub(crate) lost_edits: bool,
    pub(crate) journal_kept: bool,
    pub(crate) commits_kept: be_sdk_type_api::KeptCommits,
    pub(crate) refused_commits: Vec<CommitId>,
    pub(crate) from_file: Option<FromFileFn>,
    /// The type's merge by parts (`MergeParts`), as a `PartsMerge<C>` for its content, if it
    /// merges by parts.
    pub(crate) parts_merge: Option<std::rc::Rc<dyn std::any::Any>>,
    pub(crate) now: u64,
}

pub(crate) trait AnyReplica {
    fn start(&mut self, out: &mut Vec<Effect>);

    fn input(&mut self, now: u64, input: Input, out: &mut Vec<Effect>);

    fn tick(&mut self, now: u64, out: &mut Vec<Effect>);

    fn wake_at(&self) -> Option<u64>;

    /// The soonest of `wake_at`'s timers that keep the block in step with the server: not a
    /// heartbeat or a read's retry.
    fn in_step_at(&self) -> Option<u64>;

    fn released(&self) -> bool;

    /// What the server lacks of the edits made here, for the block's next replica.
    fn carry(&mut self) -> Carried;

    fn adopt(&mut self, carried: Carried);
}

pub(crate) enum Input {
    Message {
        from: Endpoint,
        bytes: Vec<u8>,
        /// What the server says of a peer's message.
        delivery: Option<Delivery>,
    },
    Attach {
        editor: EditorId,
        editable: bool,
    },
    Detach {
        editor: EditorId,
    },
    Head(HeadEvent),
    Lease {
        session: SessionState,
        resynchronize: bool,
    },
    Read {
        token: Token,
        result: Result<be_block::Stored, String>,
    },
    PartRead {
        token: Token,
        result: Result<Vec<u8>, String>,
    },
    PartPut {
        token: Token,
        result: Result<be_block::PartRef, String>,
    },
    Resumed {
        token: Token,
        result: Result<Resume, String>,
    },
    /// The host's answer to an append of the entries `from..=through`.
    Appended {
        from: u64,
        through: u64,
        held: bool,
    },
    Drained {
        refused: bool,
        commits: Vec<CommitId>,
    },
    Close,
    Claim,
    Change {
        id: RequestId,
        change: Change,
    },
    Content {
        id: RequestId,
    },
    Seal {
        id: RequestId,
        force: bool,
    },
}

enum Journaled<Op> {
    Edited {
        operation: Op,
        unsettled: u64,
    },
    Applied(Op),
    Beneath {
        operation: Op,
        unsettled: u64,
        local: Vec<Op>,
    },
    Replaced,
}

enum Role {
    Owner(Sequencer),
    Follower(Follower),
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Sealing {
    Due,
    Rekey,
    Once,
}

enum Job<C> {
    Input(Input),
    /// A peer's message, decoded (none when this build cannot read it).
    Heard {
        from: ClientId,
        wire: Option<Wire>,
        /// What the server checked the sender may send.
        kind: RelayKind,
    },
    Seal {
        sealing: Sealing,
        reply: Option<RequestId>,
    },
    Reconcile,
    Replace {
        content: C,
        reply: Option<RequestId>,
        outright: bool,
    },
    Keep(Keeping),
    /// A part this replica put for one of its operations was answered: the owner sequences what
    /// it held, a follower submits what it has not sent.
    Unblocked,
    /// The owner could not put the part of an operation too large to relay.
    Unput,
    Restart(CommitId),
    Release,
}

impl<C> Job<C> {
    fn passes_a_seal(&self) -> bool {
        matches!(
            self,
            Job::Input(
                Input::Message {
                    from: Endpoint::Editor(_),
                    ..
                } | Input::Attach { .. }
                    | Input::Detach { .. }
                    | Input::Content { .. }
                    | Input::Change {
                        change: Change::Child(_) | Change::Undo | Change::Redo,
                        ..
                    }
            )
        )
    }
}

#[derive(Clone, Copy)]
struct Handed {
    commit: CommitId,
    base: Option<CommitId>,
}

#[derive(Clone, Copy)]
struct Keeping {
    commit: CommitId,
    base: Option<CommitId>,
    from: ClientId,
    attempts: u8,
}

/// A part a replica puts: for an editor that asked, or the bytes of one of its own operations
/// too large to relay, by their hash.
#[derive(Clone, Copy)]
enum Putting {
    Editor { editor: EditorId, token: u64 },
    Operation(Hash),
}

/// An operation too large to relay, as the part its bytes are put as.
#[derive(Clone, Copy)]
enum Upload {
    Putting,
    Put(PartRef),
}

/// A range read, answered once every piece is.
struct RangeRead {
    reader: Reader,
    pieces: Vec<Piece>,
}

#[derive(Clone, Copy)]
enum Reader {
    Editor { editor: EditorId, token: u64 },
    Content(RequestId),
}

enum Piece {
    Ready(Vec<u8>),
    Waiting(Token),
    Failed(String),
}

enum Slot<C> {
    Waiting(Token),
    Ready(C),
}

impl Handed {
    fn message(&self) -> SessionMessage {
        SessionMessage::Kept {
            commit: self.commit,
            base: self.base,
        }
    }
}

enum Then {
    Load,
    Snapshot {
        head: Option<CommitId>,
        sealed: u64,
        sequence: u64,
        ops: Vec<SessionOp>,
        state: Vec<u8>,
        applied: u64,
    },
    SealedReload {
        head: CommitId,
        sequence: u64,
        state: Vec<u8>,
        restart: bool,
    },
    Restarted {
        head: CommitId,
    },
    Replacement,
    FastForward {
        to: CommitId,
    },
    FastForwardMerge {
        to: CommitId,
    },
    Merge {
        base: Option<CommitId>,
        ours: CommitId,
        theirs: CommitId,
        unsealed: bool,
    },
}

enum AfterPublish<C> {
    Seal {
        retry: bool,
        reply: Option<RequestId>,
    },
    Replace {
        content: C,
        attempts: u8,
        reply: Option<RequestId>,
        outright: bool,
        merging: Option<Merging>,
    },
    Merged {
        value: C,
    },
    Kept {
        keeping: Keeping,
    },
}

enum AfterStash<C> {
    Merge {
        value: C,
        expected: CommitId,
        base: Option<CommitId>,
        count: usize,
    },
    Replace {
        content: C,
        reply: Option<RequestId>,
    },
    Keep,
}

/// A type's merge by parts, over the three contents, reading what it needs with the reader.
pub(crate) type PartsMerge<C> =
    fn(
        C,
        C,
        C,
        be_block::PartReader,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<MergeResult<C>, String>>>>;

/// The part reads an asynchronous merge asked for, and what answered them.
#[derive(Default)]
struct MergeReads {
    next: u64,
    asked: Vec<(u64, PartRef, u64, u64)>,
    answered: BTreeMap<u64, Result<Vec<u8>, String>>,
    /// The waker each pending read was last polled with: a combinator that polls only the
    /// children woken needs it woken when the answer comes.
    wakers: BTreeMap<u64, std::task::Waker>,
}

/// A part read of a merge that is out: the merge's own id for it, and how often it was tried.
struct MergeRead {
    id: u64,
    part: PartRef,
    offset: u64,
    length: u64,
    tries: u8,
}

/// What a merge goes on to do once it has merged.
enum MergeThen<C> {
    /// The head moved to `to` while this replica had work the server lacks: ours is what it
    /// confirmed.
    FastForward { to: CommitId },
    Resume {
        base: Option<CommitId>,
        ours: CommitId,
        theirs: CommitId,
        unsealed: bool,
        ours_content: C,
    },
}

/// How often a part read of a merge is tried before the merge gives up and reports it.
const MERGE_READ_TRIES: u8 = 3;

/// How long a merge that could not read its parts waits to be tried again, doubling up to the
/// ceiling.
const MERGE_RETRY_INTERVAL: u64 = 5_000;
const MERGE_RETRY_CEILING: u64 = 60_000;

enum Wait<C> {
    /// A merge by parts, polled again as the parts it asked for arrive: it never resolves
    /// without them.
    Merging {
        merge: std::pin::Pin<Box<dyn std::future::Future<Output = Result<MergeResult<C>, String>>>>,
        reads: std::rc::Rc<std::cell::RefCell<MergeReads>>,
        reading: BTreeMap<Token, MergeRead>,
        then: MergeThen<C>,
    },
    Reads {
        slots: Vec<Slot<C>>,
        then: Then,
    },
    Resume {
        token: Token,
    },
    Publish {
        token: Token,
        then: AfterPublish<C>,
    },
    Stash {
        token: Token,
        then: AfterStash<C>,
    },
    /// The content just loaded, while the parts its journal's operations name are read.
    Payloads {
        loaded: C,
    },
}

#[derive(Default)]
struct Reported {
    status: Option<Status>,
    history: Option<UndoState>,
}

pub(crate) struct Replica<C: LiveEdit, H> {
    id: ReplicaId,
    content_type: Uuid,
    organization: Uuid,
    client: ClientId,
    tag: SessionTag,
    from_file: Option<FromFileFn>,
    parts_merge: Option<PartsMerge<C>>,
    state: SessionState,
    role: Role,
    confirmed: C,
    visible: C,
    unsent: VecDeque<(C::Op, usize, u64)>,
    held: VecDeque<C::Op>,
    in_flight: u64,
    covered: bool,
    base: Option<CommitId>,
    sealed: u64,
    sealed_state: Vec<u8>,
    /// The sequence and session state the seal on its way holds, when the owner is alone and
    /// keeps sequencing and journaling what is typed meanwhile (`journals_during_seal`).
    sealing_through: Option<(u64, Vec<u8>)>,
    reload: bool,
    catching_up: bool,
    presence: BTreeMap<(ClientId, Uuid), Vec<u8>>,
    shown: BTreeMap<Uuid, Vec<u8>>,
    presence_changed: bool,
    channel: Vec<PeerMessage>,
    incompatible: bool,
    other_builds: BTreeSet<ClientId>,
    diverged: bool,
    unreadable: Option<String>,
    history: H,
    editors: BTreeMap<EditorId, bool>,
    acknowledged: BTreeMap<EditorId, u64>,
    projected: Settling<C>,
    landed: Vec<(Landed<C::Op>, Option<EditorId>)>,
    resnapshot: bool,
    described: bool,
    jobs: VecDeque<Job<C>>,
    waiting: Option<Wait<C>>,
    tokens: Token,
    now: u64,
    owes_since: Option<u64>,
    last_edit: Option<u64>,
    opened_journal: Option<Journal>,
    /// The seal this device made on the head it opened at, which the server took after the
    /// journal was last rebased (`opening.sealed-head`): taken once the journal is replayed.
    sealed_head: Option<be_sdk_type_api::SealedHead>,
    rebased: bool,
    unappended: Vec<SessionOp>,
    append_at: Option<u64>,
    /// When a follower puts again the part of an operation whose put failed.
    put_retry_at: Option<u64>,
    /// When a merge that could not read its parts is tried again, and how often it failed since
    /// one resolved.
    merge_retry_at: Option<u64>,
    merge_failures: u8,
    appended_bytes: usize,
    applied_ops: Vec<SessionOp>,
    restarting: bool,
    restarted_at: Option<CommitId>,
    lost_edits: bool,
    beaten: Option<u64>,
    active: bool,
    sealing: bool,
    claimed: Option<u64>,
    replaced_at: u64,
    reached: Option<CommitId>,
    awaiting: Option<CommitId>,
    replacing: bool,
    handed: Vec<Handed>,
    saved: VecDeque<CommitId>,
    loaded: bool,
    released: bool,
    reported: Reported,
    ranges: Vec<RangeRead>,
    /// Parts this replica asked the host to put, by the token of the call.
    puts: BTreeMap<Token, Putting>,
    /// This replica's operations too large to relay, by the hash of their bytes.
    uploads: HashMap<Hash, Upload>,
    /// The bytes of operations sent by their part that this replica put or read, until it has
    /// applied them and no pending operation of its own names them.
    payloads: HashMap<PartRef, Vec<u8>>,
    /// Payload parts being read, by the token of the call.
    reading: BTreeMap<Token, PartRef>,
    /// Payload parts whose read failed: an operation naming one cannot be applied.
    /// Reads of parts that failed, and when each is tried again.
    read_failures: HashMap<PartRef, u32>,
    read_retry: HashMap<PartRef, u64>,
    /// How many catch-ups in a row were asked for because a part could not be read, and when
    /// the next is asked for if no answer came.
    catch_up_failures: u32,
    catch_up_at: Option<u64>,
    /// Senders whose owner-only messages a follower ignored because the lease it knew named
    /// another owner: when one becomes the owner, the follower asks to be caught up.
    ignored_owners: Vec<ClientId>,
    /// How many times the owner told this follower to submit again since one of its edits was
    /// last accepted.
    resends: u32,
    /// How many times in a row a put failed.
    put_failures: u32,
    /// What the server lacks of the edits made here (`ToEditor::Lacking`).
    lacking: Lacking,
    /// The journal opened on is this device's own (`opening.journal-kept`).
    journal_kept: bool,
    /// What commits of the block this device keeps (`opening.commits-kept`).
    commits_kept: be_sdk_type_api::KeptCommits,
    /// The refused commits this device holds (`opening.refused-commits`).
    refused_commits: Vec<CommitId>,
    out: Vec<Effect>,
}

impl<C, H> AnyReplica for Replica<C, H>
where
    C: LiveEdit + Merge + Clone + Default,
    H: Recorder<C>,
{
    fn start(&mut self, out: &mut Vec<Effect>) {
        match self.base {
            Some(head) => self.read(vec![Some(head)], Then::Load),
            None => self.loaded(C::default()),
        }
        self.run(out);
    }

    fn input(&mut self, now: u64, input: Input, out: &mut Vec<Effect>) {
        self.now = self.now.max(now);
        match input {
            Input::Read { token, result } => self.answer_read(token, result),
            Input::PartRead { token, result } => self.answer_part_read(token, result),
            Input::PartPut { token, result } => self.answer_part_put(token, result),
            Input::Resumed { token, result } => self.answer_resume(token, result),
            Input::Message {
                from: Endpoint::Peer(from),
                bytes,
                delivery,
            } => self.jobs.push_back(Job::Heard {
                from,
                wire: Wire::decode(&bytes),
                kind: delivery.map_or(RelayKind::Peer, |delivery| delivery.kind),
            }),
            Input::Appended {
                from,
                through,
                held,
            } => self.lacking.appended(from, through, held),
            Input::Drained { refused, commits } => self.lacking.drained(refused, &commits),
            Input::Head(
                event @ (HeadEvent::Published { .. }
                | HeadEvent::Rejected { .. }
                | HeadEvent::Stashed { .. }
                | HeadEvent::Failed { .. }),
            ) => self.answer_head(event),
            input => self.jobs.push_back(Job::Input(input)),
        }
        self.run(out);
    }

    fn tick(&mut self, now: u64, out: &mut Vec<Effect>) {
        self.now = self.now.max(now);
        if self.append_at.is_some_and(|at| self.now >= at) {
            self.append_unappended();
        }
        let due: Vec<PartRef> = self
            .read_retry
            .iter()
            .filter(|(_, at)| self.now >= **at)
            .map(|(part, _)| *part)
            .collect();
        for part in due {
            self.read_retry.remove(&part);
        }
        if self.catch_up_at.is_some_and(|at| self.now >= at) {
            self.catch_up_at = None;
            if self.catching_up {
                self.regreet();
            }
        }
        if self.put_retry_at.is_some_and(|at| self.now >= at) {
            self.put_retry_at = None;
            self.flush();
        }
        if self.merge_retry_at.is_some_and(|at| self.now >= at) {
            self.merge_retry_at = None;
            // The owner reconciles; a follower leaves that to the owner.
            if self.is_owner() {
                self.jobs.push_back(Job::Reconcile);
            }
        }
        let due = self.commit_at().is_some_and(|at| self.now >= at);
        if due && !self.sealing {
            self.sealing = true;
            self.jobs.push_back(Job::Seal {
                sealing: Sealing::Due,
                reply: None,
            });
        }
        if self.beat_at().is_some_and(|at| self.now >= at) {
            self.heartbeat();
        }
        self.run(out);
    }

    fn wake_at(&self) -> Option<u64> {
        let seal = match self.sealing {
            true => None,
            false => self.commit_at(),
        };
        seal.into_iter()
            .chain(self.append_at)
            .chain(self.put_retry_at)
            .chain(self.merge_retry_at)
            .chain(self.catch_up_at)
            .chain(self.read_retry.values().copied())
            .chain(self.beat_at())
            .min()
    }

    fn in_step_at(&self) -> Option<u64> {
        let seal = match self.sealing {
            true => None,
            false => self.commit_at(),
        };
        seal.into_iter()
            .chain(self.append_at)
            .chain(self.put_retry_at)
            .chain(self.merge_retry_at)
            .chain(self.catch_up_at)
            .min()
    }

    fn released(&self) -> bool {
        self.released
    }

    fn carry(&mut self) -> Carried {
        self.lacking.carry()
    }

    fn adopt(&mut self, carried: Carried) {
        self.lacking.adopt(carried);
        self.lacking
            .opened_with(self.commits_kept, &self.refused_commits);
    }
}

impl<C, H> Replica<C, H>
where
    C: LiveEdit + Merge + Clone + Default,
    H: Recorder<C>,
{
    pub(crate) fn new(opened: Opened) -> Self {
        let empty = C::default();
        Self {
            id: opened.replica,
            content_type: opened.content_type,
            organization: opened.organization,
            client: opened.client,
            tag: opened.tag,
            from_file: opened.from_file,
            parts_merge: opened.parts_merge.as_ref().and_then(|merge| {
                let merge = merge.downcast_ref::<PartsMerge<C>>().copied();
                debug_assert!(merge.is_some(), "a merge by parts of another content type");
                merge
            }),
            state: opened.session,
            role: Role::Follower(Follower::new(opened.client)),
            projected: Settling::new(empty.clone(), Vec::new()),
            sealed_state: Vec::new(),
            sealing_through: None,
            visible: empty.clone(),
            confirmed: empty,
            unsent: VecDeque::new(),
            held: VecDeque::new(),
            in_flight: 0,
            covered: true,
            base: opened.head,
            sealed: 0,
            reload: false,
            catching_up: false,
            presence: BTreeMap::new(),
            shown: BTreeMap::new(),
            presence_changed: false,
            channel: Vec::new(),
            incompatible: false,
            other_builds: BTreeSet::new(),
            diverged: false,
            unreadable: None,
            history: H::default(),
            editors: BTreeMap::new(),
            acknowledged: BTreeMap::new(),
            landed: Vec::new(),
            resnapshot: false,
            described: false,
            jobs: VecDeque::new(),
            waiting: None,
            tokens: 0,
            now: opened.now,
            owes_since: None,
            last_edit: None,
            opened_journal: opened.journal,
            sealed_head: opened.sealed_head,
            rebased: false,
            unappended: Vec::new(),
            append_at: None,
            put_retry_at: None,
            merge_retry_at: None,
            merge_failures: 0,
            appended_bytes: 0,
            applied_ops: Vec::new(),
            restarting: false,
            restarted_at: None,
            lost_edits: opened.lost_edits,
            beaten: None,
            active: false,
            sealing: false,
            claimed: None,
            replaced_at: 0,
            reached: None,
            awaiting: None,
            replacing: false,
            handed: Vec::new(),
            saved: VecDeque::new(),
            loaded: false,
            released: false,
            reported: Reported::default(),
            ranges: Vec::new(),
            puts: BTreeMap::new(),
            uploads: HashMap::new(),
            payloads: HashMap::new(),
            reading: BTreeMap::new(),
            read_failures: HashMap::new(),
            read_retry: HashMap::new(),
            catch_up_failures: 0,
            catch_up_at: None,
            ignored_owners: Vec::new(),
            resends: 0,
            put_failures: 0,
            lacking: Lacking::default(),
            journal_kept: opened.journal_kept,
            commits_kept: opened.commits_kept,
            refused_commits: opened.refused_commits,
            out: Vec::new(),
        }
    }

    fn run(&mut self, out: &mut Vec<Effect>) {
        loop {
            self.flush_editors();
            if self.released {
                break;
            }
            let Some(job) = self.jobs.front() else {
                break;
            };
            if self.waiting.is_some() && !(self.sealing_out() && job.passes_a_seal()) {
                break;
            }
            // A peer's message waits for the parts its operations name, and so does all that
            // came after it.
            if let Job::Heard {
                from,
                wire: Some(wire),
                kind,
            } = job
            {
                let (from, wire) = (*from, wire.clone());
                if !wire.sent_as(*kind) || self.not_heeded(from, &wire) {
                    self.jobs.pop_front();
                    continue;
                }
            }
            let Some(job) = self.jobs.front() else {
                break;
            };
            let mut missing = Vec::new();
            let mut backing_off = false;
            let mut too_large = None;
            if let Job::Heard {
                wire: Some(wire), ..
            } = job
            {
                for part in named_parts(wire) {
                    if self.payloads.contains_key(&part) {
                        continue;
                    }
                    if part.length() > LARGEST_PART {
                        too_large = Some(part);
                    } else if self.read_retry.contains_key(&part) {
                        backing_off = true;
                    } else {
                        missing.push(part);
                    }
                }
            }
            if let Some(part) = too_large {
                self.part_unavailable(part);
                continue;
            }
            if !missing.is_empty() || backing_off {
                for part in missing {
                    self.read_payload(part);
                }
                if self.jobs.len() > MOST_HELD_JOBS && !self.is_owner() {
                    self.drop_heard(|_| true);
                    self.catch_up_later();
                }
                break;
            }
            let Some(job) = self.jobs.pop_front() else {
                break;
            };
            self.job(job);
        }
        self.prune_payloads();
        match self.loaded && self.is_owner() && !self.released {
            true => {
                self.beaten.get_or_insert(self.now);
            }
            false => self.beaten = None,
        }
        self.tell_lacking();
        self.report();
        out.append(&mut self.out);
    }

    /// Whether what the owner's editors do while its seal is on the way is sequenced and
    /// journaled at once, as at any other time, rather than held until the seal is answered: so
    /// it is on this device whatever the server does. Only when the seal holds a known sequence
    /// (`sealing_through`), which `seal` records unless the seal restarts the session; then what
    /// is typed is held. Followers that applied what came after the seal reload the seal and
    /// hear it again (`settle_at`).
    fn journals_during_seal(&self) -> bool {
        self.sealing_through.is_some()
    }

    fn sealing_out(&self) -> bool {
        matches!(
            self.waiting,
            Some(Wait::Publish {
                then: AfterPublish::Seal { .. },
                ..
            })
        )
    }

    /// Sequences what the owner held back, in order, stopping at an operation whose part is
    /// still being put.
    fn sequence_held(&mut self) {
        if !self.is_owner() {
            for operation in std::mem::take(&mut self.held) {
                let size = C::encode_operation(&operation).len();
                self.unsent.push_back((operation, size, 1));
            }
            self.lacking.waiting_to_unsettled();
            return self.flush();
        }
        while !self.held.is_empty() && !self.putting() {
            let Some(payload) = self.payload_of(C::encode_operation(&self.held[0])) else {
                break;
            };
            let Some(operation) = self.held.pop_front() else {
                break;
            };
            let Role::Owner(sequencer) = &mut self.role else {
                break;
            };
            let id = OpId {
                client: self.client,
                counter: sequencer.sequence() + 1,
            };
            if let Some(op) = sequencer.accept(id, payload) {
                self.lacking.sequenced(op.sequence);
                self.confirmed.apply(&operation);
                self.sequenced(op);
            }
        }
    }

    /// Whether a part this replica puts for one of its operations is unanswered: what it holds
    /// or has not sent waits behind it.
    fn putting(&self) -> bool {
        self.uploads
            .values()
            .any(|upload| matches!(upload, Upload::Putting))
    }

    /// What an operation encoded as `bytes` goes as: the bytes, or for one too large to relay
    /// the part they were put as. None until that part is put, which this starts.
    fn payload_of(&mut self, bytes: Vec<u8>) -> Option<Payload> {
        if bytes.len() <= LARGEST_RELAYED_OPERATION {
            return Some(Payload::Inline(bytes));
        }
        let hash = Hash::of(&bytes);
        match self.uploads.get(&hash) {
            Some(Upload::Put(part)) => {
                let part = *part;
                self.uploads.remove(&hash);
                self.payloads.insert(part, bytes);
                Some(Payload::Part(part))
            }
            Some(Upload::Putting) => None,
            None => {
                let token = self.token();
                self.out.push(Effect::PutPart {
                    replica: self.id,
                    token,
                    bytes,
                });
                self.puts.insert(token, Putting::Operation(hash));
                self.uploads.insert(hash, Upload::Putting);
                None
            }
        }
    }

    /// The operation `payload` carries, once its bytes are here.
    fn operation(&self, payload: &Payload) -> Option<C::Op> {
        let bytes = match payload {
            Payload::Inline(bytes) => bytes,
            Payload::Part(part) => self.payloads.get(part)?,
        };
        C::decode_operation(bytes).ok()
    }

    fn read_payload(&mut self, part: PartRef) {
        if self.reading.values().any(|reading| *reading == part) {
            return;
        }
        let token = self.token();
        self.out.push(Effect::ReadPart {
            replica: self.id,
            token,
            part,
            offset: 0,
            length: part.length(),
        });
        self.reading.insert(token, part);
    }

    fn answer_payload(&mut self, part: PartRef, result: Result<Vec<u8>, String>) {
        match result {
            Ok(bytes) => {
                self.payloads.insert(part, bytes);
                self.read_failures.remove(&part);
            }
            Err(error) if matches!(self.waiting, Some(Wait::Payloads { .. })) => {
                self.error(format!("an operation's part could not be read: {error}"));
            }
            Err(error) => {
                let failures = self.read_failures.entry(part).or_default();
                *failures += 1;
                let failures = *failures;
                let limit = match self.is_owner() {
                    true => OWNER_READ_ATTEMPTS,
                    false => READ_ATTEMPTS,
                };
                if failures < limit {
                    let wait =
                        (READ_RETRY_INTERVAL << (failures - 1).min(6)).min(READ_RETRY_CEILING);
                    self.read_retry.insert(part, self.now + wait);
                } else {
                    self.error(format!("an operation's part could not be read: {error}"));
                    self.part_unavailable(part);
                }
            }
        }
        if self.reading.is_empty()
            && let Some(Wait::Payloads { .. }) = &self.waiting
            && let Some(Wait::Payloads { loaded }) = self.waiting.take()
        {
            self.loaded(loaded);
        }
    }

    /// A part a peer's message names cannot be read, however often it is tried. The messages
    /// that name it are dropped. A follower asks the owner to catch it up, which hands it the
    /// commit the owner made of the edit, and asks again, ever less often, until an answer
    /// applies; the owner reports it, and the sender's edit stays pending until it resynchronizes.
    /// Nothing here is a divergence: only bytes that do not decode are.
    fn part_unavailable(&mut self, part: PartRef) {
        let named = |job: &Job<C>| match job {
            Job::Heard {
                wire: Some(wire), ..
            } => named_parts(wire).contains(&part),
            _ => false,
        };
        let senders = self.drop_heard(named);
        if self.is_owner() {
            for from in senders {
                self.send(Some(from), &SessionMessage::Resend);
            }
        } else {
            self.catch_up_later();
        }
    }

    /// Drops the peer messages `drop` picks, and what was known of reading their parts.
    /// Answers who sent the `Submit`s among them.
    fn drop_heard(&mut self, pick: impl Fn(&Job<C>) -> bool) -> Vec<ClientId> {
        let mut senders = Vec::new();
        let jobs = std::mem::take(&mut self.jobs);
        for job in jobs {
            if !matches!(job, Job::Heard { .. }) || !pick(&job) {
                self.jobs.push_back(job);
                continue;
            }
            if let Job::Heard {
                from,
                wire: Some(wire),
            } = &job
            {
                if matches!(wire, Wire::Session(SessionMessage::Submit { .. }))
                    && !senders.contains(from)
                {
                    senders.push(*from);
                }
                for part in named_parts(wire) {
                    self.read_failures.remove(&part);
                    self.read_retry.remove(&part);
                }
            }
        }
        senders
    }

    /// Asks the owner to catch this follower up now if it has not been asked for lately, and
    /// again after 5 s, 10 s and so on up to a minute should no answer apply.
    fn catch_up_later(&mut self) {
        match self.catch_up_failures == 0 || self.catch_up_at.is_none() {
            true => self.regreet(),
            false => self.catching_up = true,
        }
    }

    fn regreet(&mut self) {
        let wait = (PUT_RETRY_INTERVAL << self.catch_up_failures.min(4)).min(PUT_RETRY_CEILING);
        self.catch_up();
        self.catch_up_failures += 1;
        self.catch_up_at = Some(self.now + wait);
    }

    /// Whether a follower ignores `wire` from `from`: what only the owner sends from anyone
    /// else (a viewer can relay anything), and what only the owner acts on. One from a sender
    /// that may be an owner the follower has not heard of yet is remembered, so that the
    /// follower asks to be caught up should that sender become the owner.
    fn not_heeded(&mut self, from: ClientId, wire: &Wire) -> bool {
        if !matches!(self.role, Role::Follower(_)) {
            return false;
        }
        let Wire::Session(message) = wire else {
            return false;
        };
        match message {
            SessionMessage::Submit { .. }
            | SessionMessage::Catchup { .. }
            | SessionMessage::Kept { .. } => true,
            SessionMessage::Accepted { .. }
            | SessionMessage::Snapshot { .. }
            | SessionMessage::Sealed { .. }
            | SessionMessage::Replaced { .. }
            | SessionMessage::Saved { .. }
            | SessionMessage::Resend => {
                let ignored = Some(from) != self.state.owner;
                if ignored && !self.ignored_owners.contains(&from) {
                    if self.ignored_owners.len() >= 8 {
                        self.ignored_owners.remove(0);
                    }
                    self.ignored_owners.push(from);
                }
                ignored
            }
            _ => false,
        }
    }

    /// Asks the owner for what this follower missed, and ignores what the owner sends until it
    /// answers.
    fn catch_up(&mut self) {
        if self.is_owner() {
            return;
        }
        self.catching_up = true;
        self.greet();
    }

    /// Drops the bytes of payload parts once nothing waiting or pending names them. Nothing
    /// goes while the replica waits: what it waits for may apply them (a snapshot's operations).
    fn prune_payloads(&mut self) {
        if self.payloads.is_empty() || self.waiting.is_some() {
            return;
        }
        let mut named: HashSet<PartRef> = self.reading.values().copied().collect();
        if let Role::Follower(follower) = &self.role {
            named.extend(follower.pending_parts());
        }
        for job in &self.jobs {
            if let Job::Heard {
                wire: Some(wire), ..
            } = job
            {
                named.extend(named_parts(wire));
            }
        }
        self.payloads.retain(|part, _| named.contains(part));
    }

    fn put_answered(&mut self, hash: Hash, result: Result<PartRef, String>) {
        match result {
            Ok(part) => {
                self.put_failures = 0;
                self.uploads.insert(hash, Upload::Put(part));
                self.jobs.push_back(Job::Unblocked);
            }
            Err(error) => {
                self.uploads.remove(&hash);
                self.error(format!(
                    "an edit too large to relay could not be put: {error}"
                ));
                self.jobs.push_back(Job::Unput);
            }
        }
    }

    fn unblocked(&mut self) {
        if !(self.is_owner() && self.sealing_out() && !self.journals_during_seal()) {
            self.sequence_held();
        }
    }

    /// The owner could not put an operation's part (offline, or the server does not have the
    /// block yet): it commits what it shows instead, which needs no part. A follower keeps the
    /// operation and puts it again when it next sends.
    fn unput(&mut self) {
        if !self.is_owner() {
            if !self.unsent.is_empty() {
                let wait = (PUT_RETRY_INTERVAL << self.put_failures.min(4)).min(PUT_RETRY_CEILING);
                self.put_failures += 1;
                self.put_retry_at = Some(self.now + wait);
            }
            return;
        }
        if self.held.is_empty() || self.putting() {
            return;
        }
        self.held.clear();
        self.replace(self.visible.clone(), None, false);
    }

    /// When the owner commits what it owes: after a quiet spell, at most `COMMIT_INTERVAL`
    /// after it started owing, or at once when the journal grew past `JOURNAL_BYTES`.
    fn commit_at(&self) -> Option<u64> {
        // No due seal while a merge cannot read its parts: it would publish on the old base and be
        // rejected at each tick until the merge's timer fires. The work stays in the journal.
        if self.merge_failures > 0 {
            return None;
        }
        let since = self.owes_since?;
        if self.appended_bytes >= JOURNAL_BYTES {
            return Some(since);
        }
        let quiet = self.last_edit.map_or(since, |edited| edited.max(since)) + QUIET_INTERVAL;
        Some(quiet.min(since + COMMIT_INTERVAL))
    }

    /// Commits now rather than at the next pause: before a build that cannot replay this one's
    /// journal reads the block, or when a follower needs a reload.
    fn commit_now(&mut self) {
        if self.is_owner() && !self.is_clean() && !self.sealing {
            self.sealing = true;
            self.jobs.push_back(Job::Seal {
                sealing: Sealing::Due,
                reply: None,
            });
        }
    }

    fn alone(&self) -> bool {
        self.state
            .participants
            .iter()
            .all(|participant| *participant == self.client)
    }

    /// The owner sequenced `op`: with others in the session it appends it to the journal and
    /// relays it in one call; alone it appends it within `APPEND_INTERVAL`, folded.
    fn sequenced(&mut self, op: SessionOp) {
        self.last_edit = Some(self.now);
        self.unappended.push(op);
        match self.alone() {
            true => {
                self.append_at.get_or_insert(self.now + APPEND_INTERVAL);
            }
            false => self.append_unappended(),
        }
    }

    fn append_unappended(&mut self) {
        self.append_at = None;
        let ops = std::mem::take(&mut self.unappended);
        if ops.is_empty() {
            return;
        }
        let from = ops[0].sequence;
        match self.alone() {
            true => {
                let folded = fold::<C>(ops);
                self.append(folded, false, from);
            }
            false => self.append(ops, true, from),
        }
    }

    /// Appends `ops`, which hold every operation from `from` on: a folded op holds those it
    /// absorbed.
    fn append(&mut self, ops: Vec<SessionOp>, relay: bool, from: u64) {
        if !self.is_owner() || ops.is_empty() {
            return;
        }
        if !self.rebased {
            self.rebase_journal(Vec::new());
        }
        self.appended_bytes += ops.iter().map(parted).sum::<usize>();
        let entries: Vec<JournalEntry> = ops
            .into_iter()
            .map(|op| JournalEntry {
                sequence: op.sequence,
                payload: Wire::Session(SessionMessage::Accepted { op }).encode(),
            })
            .collect();
        self.appended_bytes += entries
            .iter()
            .map(|entry| entry.payload.len())
            .sum::<usize>();
        let through = entries
            .iter()
            .map(|entry| entry.sequence)
            .max()
            .unwrap_or(from);
        let announcements = self.lacking.take_appended(from, through);
        self.out.push(Effect::Append {
            replica: self.id,
            generation: self.state.generation,
            entries,
            relay,
            from,
            announcements,
        });
    }

    /// Starts the journal from this owner's base: its last commit, the sequence number that
    /// commit holds, and the session state as of it.
    fn rebase_journal(&mut self, entries: Vec<JournalEntry>) {
        self.rebased = true;
        self.out.push(Effect::Rebase {
            replica: self.id,
            generation: self.state.generation,
            base: JournalBase {
                commit: self.base,
                sequence: self.sealed,
                state: self.sealed_state.clone(),
            },
            entries,
        });
    }

    fn beat_at(&self) -> Option<u64> {
        let busy = self.active || !self.is_clean();
        self.beaten
            .filter(|_| busy)
            .map(|beaten| beaten + HEARTBEAT_INTERVAL)
    }

    fn heartbeat(&mut self) {
        self.beaten = Some(self.now);
        self.active = false;
        let clean_at = self.base.filter(|_| self.is_clean());
        self.out.push(Effect::ReportClean {
            replica: self.id,
            generation: self.state.generation,
            clean_at,
        });
    }

    fn job(&mut self, job: Job<C>) {
        if self.unreadable.is_some() {
            return self.job_while_unreadable(job);
        }
        match job {
            Job::Input(input) => self.handle(input),
            Job::Heard { from, wire, .. } => self.heard(from, wire),
            Job::Seal { sealing, reply } => self.seal(sealing, reply),
            Job::Reconcile => self.reconcile(),
            Job::Replace {
                content,
                reply,
                outright,
            } => self.replace(content, reply, outright),
            Job::Keep(keeping) => self.keep(keeping),
            Job::Unblocked => self.unblocked(),
            Job::Unput => self.unput(),
            Job::Restart(head) => self.read(vec![Some(head)], Then::Restarted { head }),
            Job::Release => self.release(),
        }
    }

    fn job_while_unreadable(&mut self, job: Job<C>) {
        match job {
            Job::Input(Input::Close) | Job::Release => self.release(),
            Job::Input(Input::Change { id, .. }) => self.reply(id, Answer::Changed(false)),
            Job::Input(Input::Content { id }) | Job::Input(Input::Seal { id, .. }) => {
                self.reply(id, Answer::Unknown);
            }
            Job::Seal {
                reply: Some(id), ..
            }
            | Job::Replace {
                reply: Some(id), ..
            } => self.reply(id, Answer::Unknown),
            _ => {}
        }
    }

    fn release(&mut self) {
        if self.released {
            return;
        }
        if self.loaded && self.is_owner() && self.alone() && self.is_clean() && self.rebased {
            // The last one out restarts the session for free: whoever opens next loads the
            // commit afresh and numbers from zero.
            self.out.push(Effect::Rebase {
                replica: self.id,
                generation: self.state.generation,
                base: JournalBase {
                    commit: self.base,
                    sequence: 0,
                    state: Vec::new(),
                },
                entries: Vec::new(),
            });
        }
        self.out.push(Effect::Release { replica: self.id });
        self.released = true;
        for job in std::mem::take(&mut self.jobs) {
            self.job_while_unreadable(job);
        }
    }

    fn handle(&mut self, input: Input) {
        match input {
            Input::Message {
                from: Endpoint::Editor(editor),
                bytes,
                ..
            } => {
                if let Some(message) = FromEditor::decode(&bytes) {
                    self.handle_editor(editor, message);
                }
                self.reported.status = None;
            }
            Input::Message {
                from: Endpoint::Host | Endpoint::Peer(_),
                ..
            } => {}
            Input::Attach { editor, editable } => self.attach(editor, editable),
            Input::Detach { editor } => {
                self.editors.remove(&editor);
                self.lacking.unfollow(editor);
            }
            Input::Head(HeadEvent::Moved { head }) => self.moved(head),
            Input::Head(_)
            | Input::Read { .. }
            | Input::PartRead { .. }
            | Input::PartPut { .. }
            | Input::Resumed { .. }
            | Input::Appended { .. }
            | Input::Drained { .. } => {}
            Input::Lease {
                session,
                resynchronize,
            } => {
                self.adopt(session);
                if resynchronize && let Role::Follower(_) = &self.role {
                    self.catch_up_failures = 0;
                    self.catch_up_at = None;
                    self.regreet();
                }
                self.flush();
            }
            Input::Claim => self.claim(),
            Input::Close => {
                self.jobs.push_front(Job::Release);
                self.jobs.push_front(Job::Seal {
                    sealing: Sealing::Due,
                    reply: None,
                });
            }
            Input::Change { id, change } => self.change(id, change),
            Input::Content { id } => match C::LOADS {
                be_block::Loads::Whole => {
                    let bytes = self.visible.encode();
                    self.reply(id, Answer::Content(bytes));
                }
                be_block::Loads::FirstPart => self.read_range(Reader::Content(id), 0, u64::MAX),
            },
            Input::Seal { id, force } => self.jobs.push_front(Job::Seal {
                sealing: match force {
                    true => Sealing::Rekey,
                    false => Sealing::Due,
                },
                reply: Some(id),
            }),
        }
    }

    fn heard(&mut self, from: ClientId, wire: Option<Wire>) {
        let Some(wire) = wire else {
            return self.unreadable_message();
        };
        self.receive(from, wire);
        if self.waiting.is_none() {
            self.flush();
        }
    }

    fn reply(&mut self, id: RequestId, reply: Answer) {
        self.out.push(Effect::Answer { id, answer: reply });
    }

    fn error(&mut self, message: impl Into<String>) {
        self.out.push(Effect::Error {
            replica: self.id,
            message: message.into(),
        });
    }

    fn token(&mut self) -> Token {
        self.tokens += 1;
        self.tokens
    }

    fn written(&self, content: &C) -> Written {
        crate::written(content, self.organization)
    }

    fn read(&mut self, commits: Vec<Option<CommitId>>, then: Then) {
        let slots = commits
            .into_iter()
            .map(|commit| match commit {
                Some(commit) => {
                    let token = self.token();
                    self.out.push(Effect::Load {
                        replica: self.id,
                        token,
                        commit,
                        first_part: C::LOADS == be_block::Loads::FirstPart,
                    });
                    Slot::Waiting(token)
                }
                None => Slot::Ready(C::default()),
            })
            .collect();
        self.waiting = Some(Wait::Reads { slots, then });
        self.finish_reads();
    }

    fn answer_read(&mut self, token: Token, result: Result<be_block::Stored, String>) {
        let Some(Wait::Reads { slots, .. }) = &mut self.waiting else {
            return;
        };
        let Some(slot) = slots
            .iter_mut()
            .find(|slot| matches!(slot, Slot::Waiting(waiting) if *waiting == token))
        else {
            return;
        };
        let decoded =
            result.and_then(|stored| C::from_stored(&stored).map_err(|error| error.to_string()));
        match decoded {
            Ok(content) => {
                *slot = Slot::Ready(content);
                self.finish_reads();
            }
            Err(error) => {
                let Some(Wait::Reads { then, .. }) = self.waiting.take() else {
                    return;
                };
                match then {
                    Then::Load => self.unreadable = Some(error),
                    _ => self.error(error),
                }
            }
        }
    }

    fn finish_reads(&mut self) {
        let Some(Wait::Reads { slots, .. }) = &self.waiting else {
            return;
        };
        if slots.iter().any(|slot| matches!(slot, Slot::Waiting(_))) {
            return;
        }
        let Some(Wait::Reads { slots, then }) = self.waiting.take() else {
            return;
        };
        let contents = slots
            .into_iter()
            .filter_map(|slot| match slot {
                Slot::Ready(content) => Some(content),
                Slot::Waiting(_) => None,
            })
            .collect();
        self.after_reads(then, contents);
    }

    fn after_reads(&mut self, then: Then, contents: Vec<C>) {
        let mut contents = contents.into_iter();
        let mut next = || contents.next().unwrap_or_default();
        match then {
            Then::Load => self.load(next()),
            Then::Snapshot {
                head,
                sealed,
                sequence,
                ops,
                state,
                applied,
            } => {
                self.confirmed = next();
                self.base = head;
                if !state.is_empty() {
                    if self.confirmed.adopt_session_state(&state).is_err() {
                        return self.unreadable_message();
                    }
                    self.sealed_state = state;
                }
                if let Role::Follower(follower) = &mut self.role {
                    follower.set_applied(sealed);
                }
                self.applied_ops.clear();
                self.rebuild_visible();
                self.snapshot_ops(sealed, sequence, ops, applied);
                self.flush();
            }
            Then::SealedReload {
                head,
                sequence,
                state,
                restart,
            } => {
                self.catch_up_failures = 0;
                self.catch_up_at = None;
                let before = std::mem::replace(&mut self.confirmed, next());
                match state.is_empty() {
                    true => self.sealed_state = self.confirmed.session_state(),
                    false => {
                        if self.confirmed.adopt_session_state(&state).is_err() {
                            self.unreadable_message();
                        }
                        self.sealed_state = state;
                    }
                }
                if restart {
                    self.carry_pending(before);
                    self.history.restart();
                    self.restarted_at = Some(head);
                }
                if let Role::Follower(follower) = &mut self.role {
                    follower.set_applied(sequence);
                }
                self.applied_ops.clear();
                self.rebuild_visible();
                self.base = Some(head);
                self.sealed = sequence;
                if restart && let Role::Follower(follower) = &mut self.role {
                    let resubmit = follower.resynchronize(sequence);
                    let owner = self.state.owner;
                    for message in resubmit {
                        self.send(owner, &message);
                    }
                }
                self.flush();
            }
            Then::Restarted { head } => self.restarted(head, next()),
            Then::Replacement => {
                self.history.clear();
                self.replace(next(), None, true);
            }
            Then::FastForward { to } => {
                self.reached = Some(to);
                self.confirmed = next();
                self.rebuild_visible();
                self.reload = true;
                self.settle_at(to);
            }
            Then::FastForwardMerge { to } => {
                let ancestor = next();
                let theirs = next();
                let ours = self.confirmed.clone();
                self.merging(MergeThen::FastForward { to }, ancestor, ours, theirs);
            }
            Then::Merge {
                base,
                ours,
                theirs,
                unsealed,
            } => {
                let ancestor = next();
                let ours_content = match unsealed {
                    true => self.confirmed.clone(),
                    false => next(),
                };
                let theirs_content = next();
                self.merging(
                    MergeThen::Resume {
                        base,
                        ours,
                        theirs,
                        unsealed,
                        ours_content: ours_content.clone(),
                    },
                    ancestor,
                    ours_content,
                    theirs_content,
                );
            }
        }
    }

    /// Merges three contents: with the type's merge by parts when it has one, which waits while
    /// the parts it needs are read (and never resolves without them), otherwise at once.
    fn merging(&mut self, then: MergeThen<C>, base: C, ours: C, theirs: C) {
        let Some(merge) = self.parts_merge else {
            let result = C::merge3(&base, &ours, &theirs);
            return self.merged(then, result);
        };
        let reads = std::rc::Rc::new(std::cell::RefCell::new(MergeReads::default()));
        let reader = {
            let reads = std::rc::Rc::clone(&reads);
            be_block::PartReader::new(move |part, offset, length| {
                let id = {
                    let mut asked = reads.borrow_mut();
                    asked.next += 1;
                    let id = asked.next;
                    asked.asked.push((id, part, offset, length));
                    id
                };
                let reads = std::rc::Rc::clone(&reads);
                Box::pin(std::future::poll_fn(move |context| {
                    let mut reads = reads.borrow_mut();
                    match reads.answered.remove(&id) {
                        Some(answer) => {
                            reads.wakers.remove(&id);
                            std::task::Poll::Ready(answer)
                        }
                        None => {
                            reads.wakers.insert(id, context.waker().clone());
                            std::task::Poll::Pending
                        }
                    }
                }))
            })
        };
        self.waiting = Some(Wait::Merging {
            merge: merge(base, ours, theirs, reader),
            reads,
            reading: BTreeMap::new(),
            then,
        });
        self.poll_merge();
    }

    /// Polls the merge out, asking the host for the parts it asked for since.
    fn poll_merge(&mut self) {
        let Some(Wait::Merging {
            mut merge,
            reads,
            mut reading,
            then,
        }) = self.waiting.take()
        else {
            return;
        };
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        match merge.as_mut().poll(&mut context) {
            std::task::Poll::Ready(Ok(result)) => {
                self.merge_failures = 0;
                self.merge_retry_at = None;
                self.merged(then, result)
            }
            std::task::Poll::Ready(Err(error)) => self.merge_failed(error),
            std::task::Poll::Pending => {
                let asked = std::mem::take(&mut reads.borrow_mut().asked);
                if asked.is_empty() && reading.is_empty() {
                    // It waits for something that is not a part read, which nothing here will
                    // answer: report it rather than wait for ever, with the jobs behind it.
                    return self.error(
                        "a merge by parts waited for something other than a part read".to_owned(),
                    );
                }
                for (id, part, offset, length) in asked {
                    let read = MergeRead {
                        id,
                        part,
                        offset,
                        length,
                        tries: 1,
                    };
                    self.ask_part(&mut reading, read);
                }
                self.waiting = Some(Wait::Merging {
                    merge,
                    reads,
                    reading,
                    then,
                });
            }
        }
    }

    fn ask_part(&mut self, reading: &mut BTreeMap<Token, MergeRead>, read: MergeRead) {
        let token = self.token();
        self.out.push(Effect::ReadPart {
            replica: self.id,
            token,
            part: read.part,
            offset: read.offset,
            length: read.length,
        });
        reading.insert(token, read);
    }

    /// A merge that could not have its parts is dropped, so the replica goes on, and tried again
    /// on a timer that doubles up to a ceiling (a `Job::Reconcile`): it resolves only with its
    /// parts. The failure is reported once, not at each try.
    fn merge_failed(&mut self, error: String) {
        if self.merge_failures == 0 {
            self.error(error);
        }
        let wait = (MERGE_RETRY_INTERVAL << self.merge_failures.min(4)).min(MERGE_RETRY_CEILING);
        self.merge_failures = self.merge_failures.saturating_add(1);
        self.merge_retry_at = Some(self.now + wait);
    }

    /// A part the merge asked for was read, or could not be: a read that failed is tried again,
    /// and the merge that cannot have it is reported rather than resolved without it.
    fn answer_merge_read(&mut self, token: Token, result: &Result<Vec<u8>, String>) -> bool {
        let Some(Wait::Merging { reading, .. }) = &mut self.waiting else {
            return false;
        };
        let Some(read) = reading.remove(&token) else {
            return false;
        };
        if result.is_err() && read.tries < MERGE_READ_TRIES {
            let again = MergeRead {
                tries: read.tries + 1,
                ..read
            };
            let token = self.token();
            self.out.push(Effect::ReadPart {
                replica: self.id,
                token,
                part: again.part,
                offset: again.offset,
                length: again.length,
            });
            if let Some(Wait::Merging { reading, .. }) = &mut self.waiting {
                reading.insert(token, again);
            }
            return true;
        }
        if let Some(Wait::Merging { reads, .. }) = &self.waiting {
            let waker = {
                let mut reads = reads.borrow_mut();
                reads.answered.insert(read.id, result.clone());
                reads.wakers.remove(&read.id)
            };
            if let Some(waker) = waker {
                waker.wake();
            }
        }
        self.poll_merge();
        true
    }

    /// What a finished merge goes on to do.
    fn merged(&mut self, then: MergeThen<C>, result: MergeResult<C>) {
        match then {
            MergeThen::FastForward { to } => match result {
                MergeResult::Clean(value) => {
                    let content = self.written(&value);
                    self.publish(Some(to), None, content, AfterPublish::Merged { value });
                }
                MergeResult::Conflicted { value, conflicts } => {
                    let content = self.written(&self.confirmed);
                    let base = self.base;
                    self.stash(
                        base,
                        content,
                        AfterStash::Merge {
                            value,
                            expected: to,
                            base,
                            count: conflicts,
                        },
                    );
                }
            },
            MergeThen::Resume {
                base,
                ours,
                theirs,
                unsealed,
                ours_content,
            } => {
                let (value, conflicts) = match result {
                    MergeResult::Clean(value) => (value, None),
                    MergeResult::Conflicted { value, conflicts } => (value, Some(conflicts)),
                };
                match conflicts {
                    Some(count) if unsealed => {
                        let content = self.written(&ours_content);
                        self.stash(
                            Some(ours),
                            content,
                            AfterStash::Merge {
                                value,
                                expected: theirs,
                                base,
                                count,
                            },
                        );
                    }
                    conflicts => {
                        if let Some(count) = conflicts {
                            self.conflict(ours, theirs, count);
                        }
                        let content = self.written(&value);
                        self.publish(
                            Some(theirs),
                            Some(Merging { base, ours }),
                            content,
                            AfterPublish::Merged { value },
                        );
                    }
                }
            }
        }
    }

    fn conflict(&mut self, ours: CommitId, theirs: CommitId, count: usize) {
        self.out.push(Effect::Conflict {
            replica: self.id,
            ours,
            theirs,
            count,
        });
    }

    fn publish(
        &mut self,
        expected: Option<CommitId>,
        merging: Option<Merging>,
        content: Written,
        then: AfterPublish<C>,
    ) {
        let token = self.token();
        // A seal holds the session through what its owner sequenced when it began.
        let sealed_through = match (&then, &self.role) {
            (AfterPublish::Seal { .. }, Role::Owner(_)) => {
                let generation = self.state.generation;
                self.sealing_through
                    .as_ref()
                    .map(|(sequence, _)| (generation, *sequence))
            }
            _ => None,
        };
        self.out.push(Effect::Publish {
            replica: self.id,
            token,
            expected,
            merging,
            content,
            sealed_through,
        });
        self.waiting = Some(Wait::Publish { token, then });
    }

    fn stash(&mut self, parent: Option<CommitId>, content: Written, then: AfterStash<C>) {
        let token = self.token();
        self.out.push(Effect::Stash {
            replica: self.id,
            token,
            parent,
            content,
        });
        self.waiting = Some(Wait::Stash { token, then });
    }

    fn answer_resume(&mut self, token: Token, result: Result<Resume, String>) {
        if !matches!(&self.waiting, Some(Wait::Resume { token: waiting }) if *waiting == token) {
            return;
        }
        self.waiting = None;
        let resume = match result {
            Ok(resume) => resume,
            Err(error) => return self.error(error),
        };
        let unsealed = self.has_unsealed_work();
        // A resume that needs no merge settles any merge that was failing.
        if !matches!(resume, Resume::Merge { .. })
            && (!unsealed || !matches!(resume, Resume::FastForward { .. }))
        {
            self.merge_failures = 0;
            self.merge_retry_at = None;
        }
        match resume {
            Resume::Empty | Resume::UpToDate | Resume::Publish { .. } => {}
            Resume::FastForward { to } if !unsealed => {
                self.read(vec![Some(to)], Then::FastForward { to });
            }
            Resume::FastForward { to } => {
                self.read(vec![self.base, Some(to)], Then::FastForwardMerge { to });
            }
            Resume::Merge { base, ours, theirs } => {
                let commits = match unsealed {
                    true => vec![base, Some(theirs)],
                    false => vec![base, Some(ours), Some(theirs)],
                };
                self.read(
                    commits,
                    Then::Merge {
                        base,
                        ours,
                        theirs,
                        unsealed,
                    },
                );
            }
        }
    }

    fn answer_head(&mut self, event: HeadEvent) {
        let answered = match (&event, &self.waiting) {
            (
                HeadEvent::Published { token, .. }
                | HeadEvent::Rejected { token, .. }
                | HeadEvent::Failed { token, .. },
                Some(Wait::Publish { token: waiting, .. }),
            )
            | (
                HeadEvent::Stashed { token, .. } | HeadEvent::Failed { token, .. },
                Some(Wait::Stash { token: waiting, .. }),
            ) => token == waiting,
            _ => false,
        };
        if !answered {
            return;
        }
        let sealed = self.sealing_out();
        self.answer_awaited(event);
        if sealed {
            self.sequence_held();
        }
    }

    fn answer_awaited(&mut self, event: HeadEvent) {
        if !matches!(event, HeadEvent::Published { .. }) {
            self.sealing_through = None;
        }
        match (self.waiting.take(), event) {
            (Some(Wait::Publish { then, .. }), HeadEvent::Published { head, held, .. }) => {
                let sealing = matches!(then, AfterPublish::Seal { .. });
                if let AfterPublish::Kept { keeping, .. } = &then
                    && keeping.from == self.client
                    && !held
                {
                    // Kept in this device's outbox: the server holds it once that drains.
                    self.lacking.kept_here(keeping.commit);
                }
                self.published(then, Some(head));
                self.lacking.sealed(self.sealed, held);
                if !sealing {
                    self.lacking.replaced(held);
                }
            }
            (Some(Wait::Publish { then, .. }), HeadEvent::Rejected { head, .. }) => {
                self.rejected(then, head);
            }
            (Some(Wait::Stash { then, .. }), HeadEvent::Stashed { commit, .. }) => {
                self.stashed(then, commit);
            }
            (waiting, HeadEvent::Failed { error, .. }) => {
                let reply = match waiting {
                    Some(Wait::Publish {
                        then: AfterPublish::Seal { reply, .. },
                        ..
                    }) => {
                        self.sealing = false;
                        reply.map(|id| (id, Answer::Sealed))
                    }
                    Some(Wait::Publish {
                        then: AfterPublish::Replace { reply, .. },
                        ..
                    })
                    | Some(Wait::Stash {
                        then: AfterStash::Replace { reply, .. },
                        ..
                    }) => reply.map(|id| (id, Answer::Changed(false))),
                    _ => None,
                };
                if let Some((id, reply)) = reply {
                    self.reply(id, reply);
                }
                self.error(error);
            }
            _ => {}
        }
    }

    fn published(&mut self, then: AfterPublish<C>, head: Option<CommitId>) {
        match then {
            AfterPublish::Seal { reply, .. } => {
                if let Some(head) = head {
                    self.settle_at(head);
                }
                self.sealing = false;
                if let Some(id) = reply {
                    self.reply(id, Answer::Sealed);
                }
            }
            AfterPublish::Replace {
                content,
                reply,
                outright,
                ..
            } => {
                let Some(head) = head else {
                    return;
                };
                match (&mut self.role, outright) {
                    (Role::Owner(sequencer), true) => {
                        let sequence = sequencer.sequence() + 1;
                        self.role = Role::Owner(Sequencer::continuing(Some(head), sequence, 0));
                        self.replaced_at = sequence;
                        self.confirmed = content;
                        self.rebuild_visible();
                        self.reload = true;
                        self.broadcast(&SessionMessage::Replaced { head });
                        self.settle_at(head);
                    }
                    (Role::Owner(_), false) => {
                        self.confirmed = content;
                        self.rebuild_visible();
                        self.reload = true;
                        self.settle_at(head);
                    }
                    (Role::Follower(follower), true) => {
                        follower.take_pending();
                        self.unsent.clear();
                        self.in_flight = 0;
                        self.covered = true;
                        self.awaiting = Some(head);
                        self.replacing = true;
                        self.confirmed = content.clone();
                        self.replace_visible(content);
                        let owner = self.state.owner;
                        self.send(owner, &SessionMessage::Replaced { head });
                    }
                    // It lost the session while committing: the owner merges the commit when it
                    // hears of it.
                    (Role::Follower(_), false) => {
                        self.unsent.clear();
                        self.replace_visible(content);
                    }
                }
                if let Some(id) = reply {
                    self.reply(id, Answer::Changed(true));
                }
            }
            AfterPublish::Merged { value } => self.take_merged(value, head),
            AfterPublish::Kept { keeping } => {
                let Some(head) = head else {
                    return;
                };
                if self.saved.len() >= SAVED_KEPT {
                    self.saved.pop_front();
                }
                self.saved.push_back(keeping.commit);
                self.acknowledge(&keeping);
                self.settle_at(head);
            }
        }
    }

    fn stashed(&mut self, then: AfterStash<C>, commit: CommitId) {
        match then {
            AfterStash::Merge {
                value,
                expected,
                base,
                count,
            } => {
                self.conflict(commit, expected, count);
                let content = self.written(&value);
                self.publish(
                    Some(expected),
                    Some(Merging { base, ours: commit }),
                    content,
                    AfterPublish::Merged { value },
                );
            }
            AfterStash::Replace { content, reply } => {
                let merging = Merging {
                    base: self.base,
                    ours: commit,
                };
                self.publish_replacement(content, reply, true, Some(merging), self.base, 0);
            }
            AfterStash::Keep => {
                self.lacking.handed(commit);
                self.drop_unconfirmed();
                self.rebuild_visible();
                self.hand(Handed {
                    commit,
                    base: self.base,
                });
            }
        }
    }

    fn acknowledge(&mut self, keeping: &Keeping) {
        match keeping.from == self.client {
            true => {
                self.handed.retain(|handed| handed.commit != keeping.commit);
                self.lacking.saved(keeping.commit);
            }
            false => self.send(
                Some(keeping.from),
                &SessionMessage::Saved {
                    commit: keeping.commit,
                },
            ),
        }
    }

    fn drop_unconfirmed(&mut self) {
        if let Role::Follower(follower) = &mut self.role {
            follower.take_pending();
        }
        self.unsent.clear();
        self.in_flight = 0;
        self.covered = true;
    }

    fn hand(&mut self, handed: Handed) {
        self.handed.push(handed);
        let owner = self.state.owner;
        self.send(owner, &handed.message());
    }

    fn publish_kept(&mut self, keeping: Keeping) {
        let content = self.written(&self.confirmed);
        self.publish(
            self.base,
            Some(Merging {
                base: keeping.base,
                ours: keeping.commit,
            }),
            content,
            AfterPublish::Kept { keeping },
        );
    }

    fn rejected(&mut self, then: AfterPublish<C>, head: Option<CommitId>) {
        match then {
            AfterPublish::Seal { retry: true, reply } if self.merge_failures == 0 => {
                self.jobs.push_front(Job::Seal {
                    sealing: Sealing::Once,
                    reply,
                });
                self.jobs.push_front(Job::Reconcile);
            }
            // A rejected seal does not retry the merge at once while one could not read its parts:
            // that waits for its timer, and the work stays unsealed meanwhile.
            AfterPublish::Seal { reply, .. } => {
                self.sealing = false;
                if let Some(id) = reply {
                    self.reply(id, Answer::Sealed);
                }
            }
            AfterPublish::Replace {
                content,
                attempts,
                reply,
                outright,
                merging,
            } => {
                if attempts + 1 >= REPLACE_ATTEMPTS {
                    self.error(
                        "a replacement could not be saved; the block kept changing under it",
                    );
                    if let Some(id) = reply {
                        self.reply(id, Answer::Changed(false));
                    }
                    return;
                }
                self.publish_replacement(content, reply, outright, merging, head, attempts + 1);
            }
            AfterPublish::Merged { value } => self.take_merged(value, None),
            AfterPublish::Kept { keeping, .. } => {
                if keeping.attempts + 1 >= REPLACE_ATTEMPTS {
                    return self.error("an edit a follower kept could not be saved");
                }
                self.jobs.push_front(Job::Keep(Keeping {
                    attempts: keeping.attempts + 1,
                    ..keeping
                }));
                self.jobs.push_front(Job::Reconcile);
            }
        }
    }

    /// Takes the content just read once the parts its journal's operations name are read too.
    fn load(&mut self, content: C) {
        let missing: Vec<PartRef> = self
            .opened_journal
            .iter()
            .filter(|journal| journal.base.commit == self.base)
            .flat_map(|journal| &journal.entries)
            .filter_map(|entry| match Wire::decode(&entry.payload) {
                Some(Wire::Session(SessionMessage::Accepted { op })) => op.payload.part(),
                _ => None,
            })
            .filter(|part| !self.payloads.contains_key(part))
            .collect();
        if missing.is_empty() {
            return self.loaded(content);
        }
        for part in missing {
            self.read_payload(part);
        }
        self.waiting = Some(Wait::Payloads { loaded: content });
    }

    fn loaded(&mut self, content: C) {
        self.confirmed = content;
        self.sealed_state = self.confirmed.session_state();
        let journal = self
            .opened_journal
            .take()
            .filter(|journal| journal.base.commit == self.base);
        let sealed_head = self.sealed_head.take();
        let mut carried = None;
        let replayed = match (journal, sealed_head) {
            (Some(mut journal), Some(sealed)) => {
                // What the seal holds replays onto the commit it was made on; the session as of
                // its last sequence is the seal's; what came after replays on top of the seal.
                let split = journal
                    .entries
                    .iter()
                    .position(|entry| entry.sequence > sealed.through)
                    .unwrap_or(journal.entries.len());
                let later = journal.entries.split_off(split);
                let mut replayed = self.replay_journal(journal);
                self.base = Some(sealed.commit);
                self.sealed = sealed.through;
                self.sealed_state = self.confirmed.session_state();
                replayed.retain(|op| op.sequence > sealed.through);
                let after = self.replay_entries(later);
                carried = Some(after.clone());
                replayed.extend(after);
                replayed
            }
            (Some(journal), None) => self.replay_journal(journal),
            (None, _) => Vec::new(),
        };
        for op in &replayed {
            // An operation sent by its part is too large to keep: it is followed without bytes.
            let bytes = match &op.payload {
                Payload::Inline(bytes) => bytes.clone(),
                Payload::Part(_) => Vec::new(),
            };
            match self.journal_kept {
                true => self.lacking.add_replayed(bytes, op.sequence),
                false => self.lacking.journaled(&bytes, Some(op.sequence)),
            }
        }
        self.visible = self.confirmed.clone();
        self.projected = Settling::new(self.visible.clone(), Vec::new());
        let last = replayed.last().map_or(self.sealed, |op| op.sequence);
        self.role = match self.state.is_owner(self.client) {
            true => {
                let mut sequencer = Sequencer::continuing(self.base, self.sealed, 0);
                for op in replayed {
                    sequencer.replayed(op);
                }
                Role::Owner(sequencer)
            }
            false => {
                let mut follower = Follower::new(self.client);
                follower.set_applied(last);
                self.applied_ops = replayed;
                Role::Follower(follower)
            }
        };
        self.loaded = true;
        self.described = true;
        if let Some(after) = carried
            && self.is_owner()
        {
            // The journal is still on the commit the seal was made on: start it over from the
            // seal, keeping what came after, in one write.
            self.owes_since = (!after.is_empty()).then_some(self.now);
            let entries = after
                .into_iter()
                .map(|op| JournalEntry {
                    sequence: op.sequence,
                    payload: Wire::Session(SessionMessage::Accepted { op }).encode(),
                })
                .collect();
            self.rebase_journal(entries);
        }
        if let Role::Follower(_) = &self.role {
            self.regreet();
        }
        self.claim_if_abandoned();
    }

    fn is_owner(&self) -> bool {
        matches!(self.role, Role::Owner(_))
    }

    /// Loads a journal on the base just read: its session state, then every operation in it,
    /// answered in order. An owner opening with it continues from its numbering without
    /// rebasing; one that took it over rebases first (`adopt`).
    fn replay_journal(&mut self, journal: Journal) -> Vec<SessionOp> {
        if !journal.base.state.is_empty() {
            if self
                .confirmed
                .adopt_session_state(&journal.base.state)
                .is_err()
            {
                return Vec::new();
            }
            self.sealed_state = journal.base.state;
        }
        self.sealed = journal.base.sequence;
        self.rebased = self.state.is_owner(self.client);
        self.replay_entries(journal.entries)
    }

    /// Applies journaled entries in order, as replaying a journal does.
    fn replay_entries(&mut self, entries: Vec<JournalEntry>) -> Vec<SessionOp> {
        let mut replayed = Vec::new();
        for entry in entries {
            let Some(Wire::Session(SessionMessage::Accepted { op })) = Wire::decode(&entry.payload)
            else {
                continue;
            };
            let Some(operation) = self.operation(&op.payload) else {
                continue;
            };
            self.confirmed.apply(&operation);
            self.appended_bytes += entry.payload.len() + parted(&op);
            replayed.push(op);
        }
        replayed
    }

    fn is_clean(&self) -> bool {
        match &self.role {
            Role::Owner(sequencer) => sequencer.is_clean() && self.held.is_empty(),
            Role::Follower(follower) => follower.pending() == 0 && self.unsent.is_empty(),
        }
    }

    fn has_unsealed_work(&self) -> bool {
        matches!(&self.role, Role::Owner(sequencer) if !sequencer.is_clean())
            || !self.held.is_empty()
    }

    fn unsettled(&self) -> u64 {
        match &self.role {
            Role::Owner(_) => 0,
            Role::Follower(_) => {
                self.in_flight + self.unsent.iter().map(|(.., weight)| weight).sum::<u64>()
            }
        }
    }

    fn cover_if_settled(&mut self) {
        if self.unsettled() == 0 {
            self.covered = true;
        }
    }

    fn journal(&mut self, entry: Journaled<C::Op>, origin: Option<EditorId>) {
        self.described = true;
        let replaced = matches!(entry, Journaled::Replaced);
        if self.landed.len() >= JOURNAL_LIMIT || replaced {
            let adds = matches!(entry, Journaled::Edited { unsettled, .. } if unsettled > 0);
            self.covered = self.unsettled() == 0 && !adds;
            self.reproject();
            return;
        }
        if self.editors.is_empty() {
            return;
        }
        let landed = match entry {
            Journaled::Edited {
                operation,
                unsettled,
            } => Landed::On {
                operation,
                unsettled,
            },
            Journaled::Applied(operation) => Landed::On {
                operation,
                unsettled: 0,
            },
            Journaled::Beneath {
                operation,
                unsettled,
                local,
            } => Landed::Beneath {
                operation,
                unsettled,
                local,
            },
            Journaled::Replaced => return,
        };
        let kept = (!self.resnapshot).then(|| landed.clone());
        if !self.projected.land(landed) {
            return self.reproject();
        }
        if let Some(landed) = kept {
            self.landed.push((landed, origin));
        }
    }

    fn reproject(&mut self) {
        self.landed.clear();
        self.resnapshot = true;
        if !self.editors.is_empty() {
            self.projected = Settling::new(self.visible.clone(), Vec::new());
        }
    }

    fn replace_visible(&mut self, visible: C) {
        self.visible = visible;
        self.journal(Journaled::Replaced, None);
    }

    fn unreadable_message(&mut self) {
        self.incompatible = true;
        if !self.is_owner() {
            self.diverged = true;
        }
    }

    fn edit(&mut self, operation: C::Op, origin: Option<EditorId>) -> bool {
        self.active = true;
        if self.diverged {
            self.error(DIVERGED);
            return false;
        }
        self.visible.apply(&operation);
        // Asked as the edit is made, so the host keeps it with the edit from here on.
        let announcements = self.visible.announcements(&operation);
        let unsettled = match self.role {
            Role::Owner(_) => 0,
            Role::Follower(_) => self.unsettled() + 1,
        };
        self.journal(
            Journaled::Edited {
                operation: operation.clone(),
                unsettled,
            },
            origin,
        );
        let payload = C::encode_operation(&operation);
        let size = payload.len();
        let oversized = size > LARGEST_RELAYED_OPERATION;
        let sealing = self.sealing_out() && !self.journals_during_seal();
        if self.is_owner() && (oversized || sealing || !self.held.is_empty()) {
            self.lacking.add(payload, Until::Waiting, announcements);
            let first = self.held.is_empty();
            self.held.push_back(operation);
            if first && !sealing {
                self.sequence_held();
            }
            return true;
        }
        match &mut self.role {
            Role::Owner(sequencer) => {
                let id = OpId {
                    client: self.client,
                    counter: sequencer.sequence() + 1,
                };
                let Some(op) = sequencer.accept(id, Payload::Inline(payload.clone())) else {
                    return true;
                };
                self.lacking
                    .add(payload, Until::Sequenced(op.sequence), announcements);
                self.confirmed.apply(&operation);
                self.sequenced(op);
            }
            Role::Follower(follower)
                if oversized
                    || follower.pending() > 0
                    || !self.unsent.is_empty()
                    || self.awaiting.is_some() =>
            {
                self.lacking.add(payload, Until::Unsettled, announcements);
                let leftover = match self.unsent.back_mut() {
                    Some((last, bound, weight)) if *bound + size <= LARGEST_RELAYED_OPERATION => {
                        let leftover = C::absorb_operation(last, operation);
                        if leftover.is_none() {
                            *bound += size;
                            *weight += 1;
                        }
                        leftover
                    }
                    _ => Some(operation),
                };
                self.unsent
                    .extend(leftover.map(|operation| (operation, size, 1)));
                if oversized {
                    self.flush();
                }
            }
            Role::Follower(follower) => {
                self.lacking
                    .add(payload.clone(), Until::Unsettled, announcements);
                let message = follower.submit(Payload::Inline(payload));
                self.in_flight = 1;
                let owner = self.state.owner;
                self.send(owner, &message);
            }
        }
        true
    }

    fn record_and_edit(&mut self, operation: C::Op, origin: Option<EditorId>) -> bool {
        self.history
            .record(&self.visible, &operation, origin, self.now);
        self.edit(operation, origin)
    }

    fn handle_editor(&mut self, editor: EditorId, message: FromEditor) {
        let Some(&editable) = self.editors.get(&editor) else {
            return;
        };
        match message {
            FromEditor::Operate(bytes) if editable => {
                let Ok(operation) = C::decode_operation(&bytes) else {
                    return;
                };
                if self.record_and_edit(operation, Some(editor)) {
                    *self.acknowledged.entry(editor).or_default() += 1;
                }
            }
            FromEditor::FollowLacking => self.lacking.follow(editor),
            FromEditor::DismissLacking(id) => self.lacking.dismiss(id),
            FromEditor::Gesture(phase) if editable => {
                let operations = match phase {
                    Gesture::Begin => {
                        self.history.begin(Some(editor));
                        Vec::new()
                    }
                    Gesture::Commit => {
                        self.history.commit(Some(editor));
                        Vec::new()
                    }
                    Gesture::Cancel => self.history.cancel(&self.visible, Some(editor)),
                    Gesture::Break => {
                        self.history.split(Some(editor));
                        Vec::new()
                    }
                };
                for operation in operations {
                    self.edit(operation, None);
                }
            }
            FromEditor::Replace(bytes) if editable => {
                if let Ok(content) = C::decode(&bytes) {
                    self.history.clear();
                    self.jobs.push_front(Job::Replace {
                        content,
                        reply: None,
                        outright: true,
                    });
                }
            }
            FromEditor::ShowPresence { kind, value } => self.set_presence(kind, value),
            FromEditor::SendToPeers {
                to,
                channel,
                payload,
            } => self.send(to, &SessionMessage::Channel { channel, payload }),
            FromEditor::Resend => self.snapshot_to(editor),
            FromEditor::Read {
                token,
                offset,
                length,
            } => self.read_range(Reader::Editor { editor, token }, offset, length),
            FromEditor::PutPart { token, bytes } if editable => {
                let put = self.token();
                self.out.push(Effect::PutPart {
                    replica: self.id,
                    token: put,
                    bytes,
                });
                self.puts.insert(put, Putting::Editor { editor, token });
            }
            FromEditor::PutPart { token, .. } => self.tell_editor(
                editor,
                &ToEditor::PartPut {
                    token,
                    result: Err("the editor is attached read-only".to_owned()),
                },
            ),
            FromEditor::Operate(_) | FromEditor::Gesture(_) | FromEditor::Replace(_) => {}
        }
    }

    fn answer_part_put(&mut self, put: Token, result: Result<be_block::PartRef, String>) {
        match self.puts.remove(&put) {
            Some(Putting::Editor { editor, token }) => {
                self.tell_editor(editor, &ToEditor::PartPut { token, result });
            }
            Some(Putting::Operation(hash)) => self.put_answered(hash, result),
            None => {}
        }
    }

    /// Reads `length` bytes of the visible content's stored form from `offset`: the bytes it
    /// holds, and the parts it left in the store through `read-part`. An editor of a type that
    /// loads only its first part reads the rest this way.
    fn read_range(&mut self, reader: Reader, offset: u64, length: u64) {
        let end = offset.saturating_add(length);
        let mut planned = Vec::new();
        let parts = self.visible.parts();
        let mut at = 0;
        let mut leaves = vec![&parts];
        while let Some(part) = leaves.pop() {
            let start = at;
            let stop = start + part.length();
            if let be_block::Part::Node { parts, .. } = part {
                if stop > offset && start < end {
                    leaves.extend(parts.iter().rev());
                } else {
                    at = stop;
                }
                continue;
            }
            at = stop;
            if stop <= offset || start >= end {
                continue;
            }
            let from = offset.max(start) - start;
            let to = end.min(stop) - start;
            match part {
                be_block::Part::Bytes(bytes) => {
                    let from = usize::try_from(from).unwrap_or(usize::MAX);
                    let to = usize::try_from(to).unwrap_or(usize::MAX);
                    planned.push(Err(bytes[from..to].to_vec()));
                }
                be_block::Part::Reused(part) => {
                    let held = self.visible.reused_bytes(*part);
                    let span = usize::try_from(from).ok().zip(usize::try_from(to).ok());
                    match held
                        .as_deref()
                        .zip(span)
                        .and_then(|(held, (from, to))| held.get(from..to))
                    {
                        Some(bytes) => planned.push(Err(bytes.to_vec())),
                        None => planned.push(Ok((*part, from, to - from))),
                    }
                }
                be_block::Part::Node { .. } => {}
            }
        }
        drop(parts);
        let mut pieces = Vec::with_capacity(planned.len());
        for plan in planned {
            match plan {
                Err(bytes) => pieces.push(Piece::Ready(bytes)),
                Ok((part, offset, length)) => {
                    let read = self.token();
                    self.out.push(Effect::ReadPart {
                        replica: self.id,
                        token: read,
                        part,
                        offset,
                        length,
                    });
                    pieces.push(Piece::Waiting(read));
                }
            }
        }
        self.ranges.push(RangeRead { reader, pieces });
        self.finish_ranges();
    }

    fn answer_part_read(&mut self, token: Token, result: Result<Vec<u8>, String>) {
        if self.answer_merge_read(token, &result) {
            return;
        }
        if let Some(part) = self.reading.remove(&token) {
            return self.answer_payload(part, result);
        }
        for range in &mut self.ranges {
            for piece in &mut range.pieces {
                if matches!(piece, Piece::Waiting(waiting) if *waiting == token) {
                    *piece = match &result {
                        Ok(bytes) => Piece::Ready(bytes.clone()),
                        Err(error) => Piece::Failed(error.clone()),
                    };
                }
            }
        }
        self.finish_ranges();
    }

    fn finish_ranges(&mut self) {
        let (done, waiting) =
            std::mem::take(&mut self.ranges)
                .into_iter()
                .partition(|range: &RangeRead| {
                    !range
                        .pieces
                        .iter()
                        .any(|piece| matches!(piece, Piece::Waiting(_)))
                });
        self.ranges = waiting;
        for range in done {
            let mut bytes = Vec::new();
            let mut failed = None;
            for piece in range.pieces {
                match piece {
                    Piece::Ready(piece) => bytes.extend(piece),
                    Piece::Failed(error) => failed = Some(error),
                    Piece::Waiting(_) => {}
                }
            }
            let result = match failed {
                Some(error) => Err(error),
                None => Ok(bytes),
            };
            match (range.reader, result) {
                (Reader::Editor { editor, token }, result) => {
                    self.tell_editor(editor, &ToEditor::Range { token, result });
                }
                (Reader::Content(id), Ok(bytes)) => self.reply(id, Answer::Content(bytes)),
                (Reader::Content(id), Err(error)) => {
                    self.error(error);
                    self.reply(id, Answer::Unknown);
                }
            }
        }
    }

    fn change(&mut self, id: RequestId, change: Change) {
        match change {
            Change::Child(change) => {
                let Some(operations) = self.visible.child_operations(crate::be_child(change))
                else {
                    return self.reply(id, Answer::Changed(false));
                };
                for operation in operations {
                    self.record_and_edit(operation, None);
                }
                self.reply(id, Answer::Changed(true));
            }
            Change::FromFile(FileChange { name, bytes }) => {
                let current = self.visible.encode();
                let content = self
                    .from_file
                    .and_then(|from_file| from_file(Some(&current), &name, bytes, self.client))
                    .and_then(|bytes| C::decode(&bytes).ok());
                self.replace_with(id, content);
            }
            Change::Replace(bytes) => {
                let content = C::decode(&bytes).ok();
                self.replace_with(id, content);
            }
            Change::Undo => {
                let operations = self.history.undo(&self.visible);
                self.step_history(id, operations);
            }
            Change::Redo => {
                let operations = self.history.redo(&self.visible);
                self.step_history(id, operations);
            }
        }
    }

    fn step_history(&mut self, id: RequestId, operations: Vec<C::Op>) {
        let changed = !operations.is_empty();
        for operation in operations {
            self.edit(operation, None);
        }
        self.reply(id, Answer::Changed(changed));
    }

    fn replace_with(&mut self, id: RequestId, content: Option<C>) {
        let Some(content) = content else {
            return self.reply(id, Answer::Changed(false));
        };
        self.history.clear();
        self.jobs.push_front(Job::Replace {
            content,
            reply: Some(id),
            outright: true,
        });
    }

    fn attach(&mut self, editor: EditorId, editable: bool) {
        let first = self.editors.is_empty();
        if self.editors.insert(editor, editable).is_some() {
            return;
        }
        match first {
            true => self.journal(Journaled::Replaced, None),
            false => self.snapshot_to(editor),
        }
        if !self.presence.is_empty() {
            let presence = self.peer_presence();
            self.tell_editor(editor, &ToEditor::Presence(presence));
        }
    }

    fn snapshot_to(&mut self, editor: EditorId) {
        let applied = self.acknowledged.get(&editor).copied().unwrap_or_default();
        let settled = self.projected.settled();
        let bytes = settled.encode();
        let length = match C::LOADS {
            be_block::Loads::Whole => bytes.len() as u64,
            be_block::Loads::FirstPart => settled.parts().length(),
        };
        let snapshot = Snapshot {
            content_type: self.content_type,
            length,
            bytes,
            session: settled.session_state(),
            unsettled: self
                .projected
                .unsettled()
                .iter()
                .map(C::encode_operation)
                .collect(),
            applied,
        };
        self.tell_editor(editor, &ToEditor::Snapshot(snapshot));
    }

    /// Tells the editors following `ToEditor::Lacking` what changed: a follower's edits the
    /// owner accepted since, one that took the session over keeping its unaccepted edits.
    fn tell_lacking(&mut self) {
        match &self.role {
            Role::Follower(_) => {
                self.lacking.settled_to(self.unsettled());
                self.lacking.lost();
            }
            Role::Owner(_) => self.lacking.promoted(),
        }
        self.announce_held();
        for (editor, message) in self.lacking.messages() {
            self.tell_editor(editor, &message);
        }
    }

    /// Hands the host what the edits the server came to hold announce, which no append carried
    /// (a follower's, once the owner accepted them).
    fn announce_held(&mut self) {
        let announcements = self.lacking.take_held();
        if !announcements.is_empty() {
            self.out.push(Effect::Announce {
                replica: self.id,
                announcements,
            });
        }
    }

    fn tell_editor(&mut self, editor: EditorId, message: &ToEditor) {
        self.out.push(Effect::Send {
            replica: self.id,
            to: Recipients::One(Endpoint::Editor(editor)),
            kind: RelayKind::Peer,
            bytes: message.encode(),
        });
    }

    fn peer_presence(&self) -> Vec<PeerPresence> {
        self.presence
            .iter()
            .map(|((client, kind), value)| PeerPresence {
                client: *client,
                kind: *kind,
                value: value.clone(),
            })
            .collect()
    }

    fn flush_editors(&mut self) {
        let editors: Vec<EditorId> = self.editors.keys().copied().collect();
        if std::mem::take(&mut self.resnapshot) {
            self.landed.clear();
            for editor in &editors {
                self.snapshot_to(*editor);
            }
        }
        let landed = std::mem::take(&mut self.landed);
        if !landed.is_empty() {
            let encoded: Vec<(Landed<Vec<u8>>, Option<EditorId>)> = landed
                .iter()
                .map(|(landed, origin)| (landed.map(C::encode_operation), *origin))
                .collect();
            for editor in &editors {
                let operations = encoded
                    .iter()
                    .map(|(landed, origin)| {
                        let mine = *origin == Some(*editor);
                        match landed {
                            Landed::On {
                                operation,
                                unsettled,
                            } => EditorOperation {
                                operation: operation.clone(),
                                mine,
                                unsettled: *unsettled,
                                beneath: None,
                            },
                            Landed::Beneath {
                                operation,
                                unsettled,
                                local,
                            } => EditorOperation {
                                operation: operation.clone(),
                                mine,
                                unsettled: *unsettled,
                                beneath: Some(local.clone()),
                            },
                        }
                    })
                    .collect();
                self.tell_editor(*editor, &ToEditor::Operations(operations));
            }
        }
        if std::mem::take(&mut self.presence_changed) && !editors.is_empty() {
            let presence = self.peer_presence();
            for editor in &editors {
                self.tell_editor(*editor, &ToEditor::Presence(presence.clone()));
            }
        }
        let channel = std::mem::take(&mut self.channel);
        if !channel.is_empty() {
            for editor in &editors {
                self.tell_editor(*editor, &ToEditor::PeerMessages(channel.clone()));
            }
        }
    }

    fn report(&mut self) {
        let status = Status {
            head: self.base,
            owner: self.is_owner(),
            clean: self.is_clean(),
            journaled: match self.role {
                Role::Owner(_) => self.unappended.is_empty() && self.held.is_empty(),
                Role::Follower(_) => self.is_clean(),
            },
            owes_seal: self.loaded && self.is_owner() && !self.is_clean(),
            incompatible: self.incompatible,
            diverged: self.diverged,
            unreadable: self.unreadable.clone(),
        };
        self.owes_since = match status.owes_seal {
            true => self.owes_since.or(Some(self.now)),
            false => None,
        };
        if self.reported.status.as_ref() != Some(&status) {
            self.reported.status = Some(status.clone());
            self.out.push(Effect::Status {
                replica: self.id,
                status,
            });
        }
        if !self.loaded || self.released {
            return;
        }
        let history = UndoState {
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
            restarted_at: self.restarted_at.filter(|_| self.history.stopped()),
            lost_edits: self.lost_edits,
        };
        if self.reported.history.as_ref() != Some(&history) {
            self.reported.history = Some(history.clone());
            self.out.push(Effect::History {
                replica: self.id,
                history,
            });
        }
        if std::mem::take(&mut self.described) {
            self.out.push(Effect::Described {
                replica: self.id,
                description: crate::describe(&self.visible, self.organization),
            });
        }
    }

    fn set_presence(&mut self, kind: Uuid, value: Option<Vec<u8>>) {
        let unchanged = match &value {
            Some(value) => self.shown.get(&kind) == Some(value),
            None => !self.shown.contains_key(&kind),
        };
        if unchanged {
            return;
        }
        match &value {
            Some(value) => self.shown.insert(kind, value.clone()),
            None => self.shown.remove(&kind),
        };
        self.broadcast(&SessionMessage::Presence { kind, value });
    }

    fn show_presence_to(&mut self, client: ClientId) {
        let shown: Vec<(Uuid, Vec<u8>)> = self
            .shown
            .iter()
            .map(|(kind, value)| (*kind, value.clone()))
            .collect();
        for (kind, value) in shown {
            self.send(
                Some(client),
                &SessionMessage::Presence {
                    kind,
                    value: Some(value),
                },
            );
        }
    }

    fn broadcast(&mut self, message: &SessionMessage) {
        self.send(None, message);
    }

    fn send(&mut self, to: Option<ClientId>, message: &SessionMessage) {
        self.send_wire(to, &Wire::Session(message.clone()));
    }

    fn send_wire(&mut self, to: Option<ClientId>, wire: &Wire) {
        self.out.push(Effect::Send {
            replica: self.id,
            to: match to {
                Some(client) => Recipients::One(Endpoint::Peer(client)),
                None => Recipients::Peers,
            },
            kind: wire.kind(),
            bytes: wire.encode(),
        });
    }

    fn greet(&mut self) {
        let Role::Follower(follower) = &self.role else {
            return;
        };
        let message = follower.catchup(self.base, self.sealed);
        let owner = self.state.owner;
        self.send_wire(owner, &Wire::Tag(self.tag));
        self.send_wire(owner, &Wire::Session(message));
    }

    fn receive(&mut self, from: ClientId, wire: Wire) {
        let message = match wire {
            Wire::Session(message) => message,
            Wire::Tag(tag) => {
                if !self.is_owner() {
                    return;
                }
                if tag == self.tag {
                    self.other_builds.remove(&from);
                    return;
                }
                self.other_builds.insert(from);
                self.commit_now();
                return self.send_wire(Some(from), &Wire::Incompatible);
            }
            Wire::Incompatible => {
                self.unreadable_message();
                return;
            }
        };
        if self.other_builds.contains(&from)
            && matches!(
                message,
                SessionMessage::Submit { .. }
                    | SessionMessage::Catchup { .. }
                    | SessionMessage::Replaced { .. }
                    | SessionMessage::Kept { .. }
            )
        {
            return;
        }
        match message {
            SessionMessage::Submit { id, base, payload } => {
                if !self.is_owner() || base < self.replaced_at {
                    return;
                }
                let operation = match self.operation(&payload) {
                    Some(operation) => operation,
                    // Its part could not be read, which was reported.
                    None if payload.part().is_some() => return,
                    None => return self.unreadable_message(),
                };
                let Role::Owner(sequencer) = &mut self.role else {
                    return;
                };
                let Some(op) = sequencer.accept(id, payload) else {
                    return;
                };
                self.confirmed.apply(&operation);
                self.visible.apply(&operation);
                self.journal(Journaled::Applied(operation), None);
                self.sequenced(op);
            }
            SessionMessage::Accepted { op } => match self.role {
                Role::Owner(_) => self.inherit_accepted(&op),
                Role::Follower(_) if !self.catching_up => self.apply_accepted(&op),
                Role::Follower(_) => {}
            },
            SessionMessage::Catchup {
                since,
                base,
                sealed,
            } => {
                let Role::Owner(sequencer) = &self.role else {
                    return;
                };
                let inherited = sequencer.inherited() > 0;
                if inherited {
                    self.reload = true;
                }
                let holds = base == sequencer.head() && sealed == self.sealed && since >= sealed;
                let message = SessionMessage::Snapshot {
                    head: sequencer.head(),
                    sealed: self.sealed,
                    sequence: sequencer.sequence(),
                    ops: sequencer.since(since.min(self.sealed)),
                    state: match holds {
                        true => Vec::new(),
                        false => self.sealed_state.clone(),
                    },
                };
                self.send(Some(from), &message);
                if inherited {
                    self.commit_now();
                }
            }
            SessionMessage::Snapshot {
                head,
                sealed,
                sequence,
                ops,
                state,
            } => {
                let Role::Follower(follower) = &self.role else {
                    return;
                };
                self.catching_up = false;
                self.catch_up_failures = 0;
                self.resends = 0;
                self.catch_up_at = None;
                let applied = follower.applied();
                if head != self.base || sealed != self.sealed || applied < sealed {
                    return self.read(
                        vec![head],
                        Then::Snapshot {
                            head,
                            sealed,
                            sequence,
                            ops,
                            state,
                            applied,
                        },
                    );
                }
                self.snapshot_ops(sealed, sequence, ops, applied);
            }
            SessionMessage::Replaced { head } => self.replaced(head),
            SessionMessage::Kept { commit, base } => {
                if self.is_owner() {
                    self.jobs.push_front(Job::Keep(Keeping {
                        commit,
                        base,
                        from,
                        attempts: 0,
                    }));
                }
            }
            SessionMessage::Resend => {
                let Role::Follower(follower) = &mut self.role else {
                    return;
                };
                self.resends += 1;
                if self.resends > MOST_RESENDS {
                    if self.resends == MOST_RESENDS + 1 {
                        self.error("the owner could not read an edit, however often it was sent");
                    }
                    return;
                }
                let applied = follower.applied();
                let owner = self.state.owner;
                for message in follower.resynchronize(applied) {
                    self.send(owner, &message);
                }
            }
            SessionMessage::Saved { commit } => {
                self.handed.retain(|handed| handed.commit != commit);
                self.lacking.saved(commit);
            }
            SessionMessage::Presence { kind, value } => {
                if from == self.client {
                    return;
                }
                let changed = match value {
                    Some(value) => self.presence.insert((from, kind), value.clone()) != Some(value),
                    None => self.presence.remove(&(from, kind)).is_some(),
                };
                self.presence_changed |= changed;
            }
            SessionMessage::Channel { channel, payload } => {
                if from != self.client {
                    self.channel.push(PeerMessage {
                        from,
                        channel,
                        payload,
                    });
                }
            }
            SessionMessage::Sealed {
                head,
                sequence,
                reload,
                restart,
            } => {
                let Role::Follower(follower) = &self.role else {
                    return;
                };
                if let Some(awaiting) = self.awaiting {
                    if awaiting != head {
                        return;
                    }
                    self.awaiting = None;
                    self.replacing = false;
                }
                if reload.is_none() && follower.applied() != sequence {
                    return self.greet();
                }
                match reload {
                    Some(state) => {
                        return self.read(
                            vec![Some(head)],
                            Then::SealedReload {
                                head,
                                sequence,
                                state,
                                restart,
                            },
                        );
                    }
                    None => self.sealed_state = self.confirmed.session_state(),
                }
                self.base = Some(head);
                self.sealed = sequence;
                self.applied_ops.retain(|op| op.sequence > sequence);
            }
        }
    }

    fn snapshot_ops(&mut self, sealed: u64, sequence: u64, ops: Vec<SessionOp>, applied: u64) {
        self.sealed = sealed;
        for op in &ops {
            self.apply_accepted(op);
        }
        let Role::Follower(follower) = &mut self.role else {
            return;
        };
        if ops.iter().all(|op| op.sequence <= applied) {
            let resubmit = follower.resynchronize(sequence);
            let owner = self.state.owner;
            for message in resubmit {
                self.send(owner, &message);
            }
        }
    }

    /// Shows what is confirmed with this replica's own edits on top: a follower's pending and
    /// unsent ones, what an owner holds back.
    fn rebuild_visible(&mut self) {
        let mut visible = self.confirmed.clone();
        if let Role::Follower(follower) = &self.role {
            for payload in follower.payloads() {
                if let Some(pending) = self.operation(&payload) {
                    visible.apply(&pending);
                }
            }
        }
        for (operation, ..) in &self.unsent {
            visible.apply(operation);
        }
        for operation in &self.held {
            visible.apply(operation);
        }
        self.replace_visible(visible);
    }

    fn flush(&mut self) {
        let Role::Follower(follower) = &mut self.role else {
            return;
        };
        if follower.pending() > 0 || self.awaiting.is_some() || self.putting() {
            return;
        }
        let Some((operation, _, weight)) = self.unsent.front() else {
            return;
        };
        let weight = *weight;
        let Some(payload) = self.payload_of(C::encode_operation(operation)) else {
            return;
        };
        self.unsent.pop_front();
        let Role::Follower(follower) = &mut self.role else {
            return;
        };
        let message = follower.submit(payload);
        self.in_flight = weight;
        let owner = self.state.owner;
        self.send(owner, &message);
    }

    fn inherit_accepted(&mut self, op: &SessionOp) {
        let Role::Owner(sequencer) = &mut self.role else {
            return;
        };
        let inherited = sequencer.inherit(op);
        if matches!(inherited, Inherited::Renumbered | Inherited::Unnumbered) {
            sequencer.carry_unsealed();
            self.reload = true;
        }
        if matches!(inherited, Inherited::Duplicate | Inherited::Renumbered) {
            return;
        }
        let Some(operation) = self.operation(&op.payload) else {
            return self.unreadable_message();
        };
        if inherited == Inherited::Continued {
            self.append_unappended();
            self.append(vec![op.clone()], false, op.sequence);
        }
        self.confirmed.apply(&operation);
        self.visible.apply(&operation);
        self.journal(Journaled::Applied(operation), None);
    }

    fn apply_accepted(&mut self, op: &SessionOp) {
        let Role::Follower(follower) = &self.role else {
            return;
        };
        if op.sequence <= follower.applied() || self.awaiting.is_some() {
            return;
        }
        let Some(operation) = self.operation(&op.payload) else {
            return self.unreadable_message();
        };
        self.applied_ops.push(op.clone());
        let Role::Follower(follower) = &mut self.role else {
            return;
        };
        if follower.is_mine(op.id) {
            self.resends = 0;
        }
        let unchanged = follower.is_mine(op.id)
            && follower.front() == Some(op.id)
            && follower
                .payloads()
                .first()
                .is_some_and(|payload| *payload == op.payload);
        if unchanged {
            follower.accepted(op);
            self.confirmed.apply(&operation);
            self.in_flight = 0;
            self.cover_if_settled();
            return;
        }
        if !follower.is_mine(op.id) && follower.pending() == 0 && self.unsent.is_empty() {
            follower.accepted(op);
            self.confirmed.apply(&operation);
            self.visible.apply(&operation);
            self.journal(Journaled::Applied(operation), None);
            return;
        }
        if follower.is_mine(op.id) {
            follower.accepted(op);
            self.confirmed.apply(&operation);
            self.in_flight = 0;
            self.rebuild_visible();
            self.cover_if_settled();
            return;
        }
        let unsettled = self.unsettled();
        let Role::Follower(follower) = &mut self.role else {
            return;
        };
        follower.accepted(op);
        if !self.covered {
            self.confirmed.apply(&operation);
            self.rebuild_visible();
            return;
        }
        let mut local: Vec<C::Op> = follower
            .payloads()
            .iter()
            .filter_map(|payload| self.operation(payload))
            .collect();
        self.in_flight = local.len() as u64;
        for (pending, _, weight) in &mut self.unsent {
            local.push(pending.clone());
            *weight = 1;
        }
        self.visible
            .apply_beneath(&mut self.confirmed, &operation, &local, &mut Vec::new());
        self.journal(
            Journaled::Beneath {
                operation,
                unsettled,
                local,
            },
            None,
        );
    }

    fn adopt(&mut self, state: SessionState) {
        let owner_changed = state.owner != self.state.owner;
        let known: BTreeSet<ClientId> = self.state.participants.iter().copied().collect();
        let joined: Vec<ClientId> = state
            .participants
            .iter()
            .copied()
            .filter(|client| *client != self.client && !known.contains(client))
            .collect();
        let before = self.presence.len();
        self.presence
            .retain(|(client, _), _| state.participants.contains(client));
        self.presence_changed |= self.presence.len() != before;
        self.state = state;
        if !self.alone() {
            self.append_unappended();
        }
        for client in joined {
            self.show_presence_to(client);
        }
        self.claim_if_abandoned();
        if !owner_changed {
            return;
        }
        let follower = match &mut self.role {
            Role::Owner(_) if self.state.is_owner(self.client) => return,
            Role::Owner(sequencer) => {
                let sequence = sequencer.sequence();
                self.role = Role::Follower(Follower::continuing(self.client, sequence));
                self.applied_ops.clear();
                self.reload = false;
                self.catch_up_failures = 0;
                self.resends = 0;
                self.catch_up_at = None;
                // A merge that was failing is the new owner's to do.
                self.merge_failures = 0;
                self.merge_retry_at = None;
                // What it held back while a part was put it now submits.
                return self.sequence_held();
            }
            Role::Follower(follower) => follower,
        };
        let replacement = self.awaiting.take().filter(|_| self.replacing);
        self.replacing = false;
        let missed = self
            .state
            .owner
            .is_some_and(|owner| self.ignored_owners.contains(&owner));
        self.ignored_owners.clear();
        self.catch_up_failures = 0;
        self.resends = 0;
        self.catch_up_at = None;
        if missed {
            self.catching_up = true;
        }
        if !self.state.is_owner(self.client) {
            let applied = follower.applied();
            let resubmit = follower.resynchronize(applied);
            let owner = self.state.owner;
            for message in resubmit {
                self.send(owner, &message);
            }
            if let Some(head) = replacement {
                self.awaiting = Some(head);
                self.replacing = true;
                self.send(owner, &SessionMessage::Replaced { head });
            }
            for handed in self.handed.clone() {
                self.send(owner, &handed.message());
            }
            if self.catching_up {
                self.regreet();
            }
            return;
        }
        let applied = follower.applied();
        let pending = follower.take_pending();
        let kept = std::mem::take(&mut self.applied_ops);
        let complete = kept.iter().all(|op| op.sequence > self.sealed)
            && kept
                .last()
                .map_or(applied == self.sealed, |op| op.sequence == applied);
        let (mut sequencer, kept) = match complete {
            true => {
                let mut sequencer = Sequencer::continuing(self.base, self.sealed, 0);
                for op in &kept {
                    sequencer.replayed(op.clone());
                }
                (sequencer, kept)
            }
            false => (
                Sequencer::continuing(self.base, applied, applied.saturating_sub(self.sealed)),
                Vec::new(),
            ),
        };
        self.catch_up_failures = 0;
        self.catch_up_at = None;
        if std::mem::take(&mut self.catching_up) {
            sequencer.carry_unsealed();
            self.reload = true;
        }
        let mut accepted = Vec::new();
        for (id, payload) in pending {
            let Some(operation) = self.operation(&payload) else {
                continue;
            };
            if let Some(op) = sequencer.accept(id, payload) {
                self.confirmed.apply(&operation);
                accepted.push(op);
            }
        }
        self.in_flight = 0;
        // What it had not sent it sequences as its own, holding back from the first that waits
        // for its part to be put.
        let mut unsent = std::mem::take(&mut self.unsent).into_iter();
        for (operation, bound, _) in unsent.by_ref() {
            if bound > LARGEST_RELAYED_OPERATION {
                self.held.push_back(operation);
                break;
            }
            let id = OpId {
                client: self.client,
                counter: sequencer.sequence() + 1,
            };
            let payload = Payload::Inline(C::encode_operation(&operation));
            if let Some(op) = sequencer.accept(id, payload) {
                self.confirmed.apply(&operation);
                accepted.push(op);
            }
        }
        self.held.extend(unsent.map(|(operation, ..)| operation));
        let inherited = sequencer.inherited() > 0;
        self.role = Role::Owner(sequencer);
        self.rebased = false;
        self.appended_bytes = 0;
        let from = kept.first().map_or(0, |op| op.sequence);
        self.append(kept, false, from);
        self.rebuild_visible();
        for handed in self.handed.clone() {
            self.jobs.push_back(Job::Keep(Keeping {
                commit: handed.commit,
                base: handed.base,
                from: self.client,
                attempts: 0,
            }));
        }
        if !self.held.is_empty() {
            self.jobs.push_back(Job::Unblocked);
        }
        match replacement {
            Some(head) => {
                let content = self.confirmed.clone();
                self.publish_replacement(content, None, true, None, Some(head), 0);
            }
            None => {
                for op in accepted {
                    self.sequenced(op);
                }
            }
        }
        // The journal started over without what this owner holds but could not number: it is
        // durable once committed.
        if inherited {
            self.commit_now();
        }
    }

    fn claim(&mut self) {
        if !self.loaded || self.catching_up || self.is_owner() {
            return;
        }
        self.claimed = Some(self.state.generation);
        self.out.push(Effect::ClaimLease {
            replica: self.id,
            generation: self.state.generation,
        });
    }

    fn claim_if_abandoned(&mut self) {
        if !self.loaded {
            return;
        }
        let held = self
            .state
            .owner
            .is_some_and(|owner| self.state.participants.contains(&owner));
        if held || self.claimed == Some(self.state.generation) {
            return;
        }
        self.claimed = Some(self.state.generation);
        self.out.push(Effect::ClaimLease {
            replica: self.id,
            generation: self.state.generation,
        });
    }

    /// A head landed: the owner reconciles with it. A follower leaves that to the owner, which
    /// hears of every head too.
    fn moved(&mut self, head: CommitId) {
        if self.base != Some(head) && self.is_owner() {
            self.jobs.push_front(Job::Reconcile);
        }
    }

    fn replaced(&mut self, head: CommitId) {
        if self.is_owner() {
            let taken = self.base == Some(head) && self.is_clean();
            let reached =
                self.reached == Some(head) && (self.base != Some(head) || self.is_clean());
            if !taken && !reached {
                self.read(vec![Some(head)], Then::Replacement);
            }
            return;
        }
        let awaited = self.awaiting.replace(head).is_some();
        self.replacing = false;
        self.history.clear();
        if !awaited && self.has_unsaved() {
            let content = self.written(&self.visible);
            self.stash(self.base, content, AfterStash::Keep);
        }
    }

    fn keep(&mut self, keeping: Keeping) {
        if !self.is_owner() {
            return;
        }
        if self.saved.contains(&keeping.commit) {
            return self.acknowledge(&keeping);
        }
        self.publish_kept(keeping);
    }

    fn has_unsaved(&self) -> bool {
        match &self.role {
            Role::Owner(sequencer) => !sequencer.is_clean() || !self.held.is_empty(),
            Role::Follower(follower) => !self.is_clean() || follower.applied() > self.sealed,
        }
    }

    fn reconcile(&mut self) {
        let token = self.token();
        self.out.push(Effect::Resume {
            replica: self.id,
            token,
            local: self.base,
        });
        self.waiting = Some(Wait::Resume { token });
    }

    fn take_merged(&mut self, value: C, published: Option<CommitId>) {
        self.confirmed = value;
        self.rebuild_visible();
        self.reload = true;
        match published {
            Some(head) => self.settle_at(head),
            None => {
                if let Role::Owner(sequencer) = &mut self.role {
                    sequencer.carry_unsealed();
                }
            }
        }
    }

    fn seal(&mut self, sealing: Sealing, reply: Option<RequestId>) {
        let skip = match sealing {
            Sealing::Due => !self.is_owner() || self.is_clean(),
            Sealing::Rekey => !self.is_owner() || self.diverged,
            Sealing::Once => false,
        };
        if skip {
            self.sealing = false;
            if let Some(id) = reply {
                self.reply(id, Answer::Sealed);
            }
            return;
        }
        if self.diverged {
            self.sealing = false;
            self.error(DIVERGED);
            if let Some(id) = reply {
                self.reply(id, Answer::Sealed);
            }
            return;
        }
        if !self.is_owner() {
            return self.rejected(
                AfterPublish::Seal {
                    retry: false,
                    reply,
                },
                self.base,
            );
        }
        // What it sequenced reaches the journal first, with what it announces, which the host
        // then keeps with this commit (`rebase-journal`).
        self.append_unappended();
        self.restarting = self.restart_due();
        self.sealing_through = match (&self.role, self.restarting) {
            (Role::Owner(sequencer), false) => {
                Some((sequencer.sequence(), self.confirmed.session_state()))
            }
            _ => None,
        };
        let content = self.written(&self.confirmed);
        self.publish(
            self.base,
            None,
            content,
            AfterPublish::Seal {
                retry: sealing != Sealing::Once,
                reply,
            },
        );
    }

    fn replace(&mut self, content: C, reply: Option<RequestId>, outright: bool) {
        if self.diverged {
            self.error(DIVERGED);
            if let Some(id) = reply {
                self.reply(id, Answer::Changed(false));
            }
            return;
        }
        // An outright replace keeps what it replaces as its merge's second parent, which history
        // never prunes: unsaved edits, and on a follower also the saved head, which only the
        // owner can seal as a bookmark (`Replicas::bookmark`).
        if outright && (self.has_unsaved() || !self.is_owner()) {
            let kept = match self.role {
                Role::Owner(_) => self.written(&self.confirmed),
                Role::Follower(_) => self.written(&self.visible),
            };
            return self.stash(self.base, kept, AfterStash::Replace { content, reply });
        }
        self.publish_replacement(content, reply, outright, None, self.base, 0);
    }

    fn publish_replacement(
        &mut self,
        content: C,
        reply: Option<RequestId>,
        outright: bool,
        merging: Option<Merging>,
        expected: Option<CommitId>,
        attempts: u8,
    ) {
        let written = self.written(&content);
        self.publish(
            expected,
            merging,
            written,
            AfterPublish::Replace {
                content,
                attempts,
                reply,
                outright,
                merging,
            },
        );
    }

    fn settle_at(&mut self, head: CommitId) {
        self.base = Some(head);
        let Role::Owner(sequencer) = &mut self.role else {
            return;
        };
        let (sequence, state) = match self.sealing_through.take() {
            Some(through) => through,
            None => (sequencer.sequence(), self.confirmed.session_state()),
        };
        sequencer.sealed_through(head, sequence);
        // What was sequenced while the seal was on its way: journaled again over the new base.
        let after = sequencer.since(sequence);
        self.sealed = sequence;
        self.sealed_state = state;
        // What came after the seal still owes one, whatever the host reports.
        self.owes_since = (!after.is_empty()).then_some(self.now);
        self.unappended.clear();
        self.append_at = None;
        self.appended_bytes = after.iter().map(parted).sum::<usize>();
        let resent = after.clone();
        let carried: Vec<JournalEntry> = after
            .into_iter()
            .map(|op| JournalEntry {
                sequence: op.sequence,
                payload: Wire::Session(SessionMessage::Accepted { op }).encode(),
            })
            .collect();
        self.appended_bytes += carried
            .iter()
            .map(|entry| entry.payload.len())
            .sum::<usize>();
        let clean = carried.is_empty();
        let restart = std::mem::take(&mut self.restarting);
        let reload = match restart {
            true => {
                self.reload = false;
                Some(Vec::new())
            }
            false => {
                // One write: the new base and what came after it, so no crash between them
                // loses what was typed while the seal was on its way.
                self.rebase_journal(carried);
                // Followers applied what came after the seal already: they load the seal with
                // its session as of `sequence`, and hear what came after again on top of it.
                (std::mem::take(&mut self.reload) || !clean).then(|| self.sealed_state.clone())
            }
        };
        let reloads = reload.is_some();
        self.broadcast(&SessionMessage::Sealed {
            head,
            sequence,
            reload,
            restart,
        });
        if reloads && !restart {
            for op in resent {
                self.broadcast(&SessionMessage::Accepted { op });
            }
        }
        self.beaten = Some(self.now);
        self.active = false;
        // Clean only with nothing past the seal: a takeover reads `clean_at` as nothing unsealed.
        if clean {
            self.out.push(Effect::ReportClean {
                replica: self.id,
                generation: self.state.generation,
                clean_at: Some(head),
            });
        }
        if restart {
            self.replaced_at = sequence;
            self.jobs.push_front(Job::Restart(head));
        }
    }

    /// Whether this commit should restart the session: the session state, which only
    /// concurrent edits need, has grown past `RESTART_STATE_BYTES` and past the content it
    /// describes, no gesture is open, and the owner is alone or the state passed the ceiling.
    fn restart_due(&self) -> bool {
        if self.history.gesture_open() {
            return false;
        }
        let state = self.confirmed.session_state().len();
        let limit = restart_limit();
        state > limit
            && state > self.confirmed.encode().len()
            && (self.alone() || state > limit.saturating_mul(RESTART_CEILING))
    }

    /// The owner restarted the session at `head`: it reloads from the commit, so every replica
    /// holds the same positions, moves what it holds back onto it, and undo stops here.
    fn restarted(&mut self, head: CommitId, fresh: C) {
        if self.base != Some(head) {
            return;
        }
        let mut before = self.confirmed.clone();
        let mut after = fresh.clone();
        let mut carried = VecDeque::new();
        for held in std::mem::take(&mut self.held) {
            let moved = C::carry_over(&held, &before, &after, self.client);
            before.apply(&held);
            if let Some(moved) = moved {
                after.apply(&moved);
                carried.push_back(moved);
            }
        }
        self.held = carried;
        self.confirmed = fresh;
        self.sealed_state = self.confirmed.session_state();
        self.rebuild_visible();
        self.history.restart();
        self.restarted_at = Some(head);
        self.rebase_journal(Vec::new());
    }

    /// Moves a follower's pending and unsent edits from `before` onto the content it just
    /// reloaded at a restart.
    fn carry_pending(&mut self, before: C) {
        let mut before = before;
        let mut after = self.confirmed.clone();
        let pending = match &mut self.role {
            Role::Follower(follower) => follower.take_pending(),
            Role::Owner(_) => Vec::new(),
        };
        let mut carried = Vec::with_capacity(pending.len());
        // From the first carried operation that needs its part put again, the rest go back to
        // being unsent, in order.
        let mut unsent = VecDeque::new();
        for (id, payload) in pending {
            let Some(operation) = self.operation(&payload) else {
                continue;
            };
            let moved = C::carry_over(&operation, &before, &after, self.client);
            before.apply(&operation);
            let Some(moved) = moved else {
                continue;
            };
            after.apply(&moved);
            let bytes = C::encode_operation(&moved);
            let size = bytes.len();
            let payload = match payload {
                Payload::Part(part) if self.payloads.get(&part) == Some(&bytes) => Some(payload),
                _ if size <= LARGEST_RELAYED_OPERATION => Some(Payload::Inline(bytes)),
                _ => None,
            };
            match payload {
                Some(payload) if unsent.is_empty() => carried.push((id, payload)),
                _ => unsent.push_back((moved, size, 1)),
            }
        }
        if let Role::Follower(follower) = &mut self.role {
            follower.restore_pending(carried);
        }
        let kept = std::mem::replace(&mut self.unsent, unsent);
        for (operation, bound, weight) in kept {
            let moved = C::carry_over(&operation, &before, &after, self.client);
            before.apply(&operation);
            if let Some(moved) = moved {
                after.apply(&moved);
                self.unsent.push_back((moved, bound, weight));
            }
        }
    }
}

#[cfg(not(test))]
fn restart_limit() -> usize {
    RESTART_STATE_BYTES
}

#[cfg(test)]
thread_local! {
    pub(crate) static RESTART_LIMIT: std::cell::Cell<usize> =
        const { std::cell::Cell::new(RESTART_STATE_BYTES) };
}

#[cfg(test)]
fn restart_limit() -> usize {
    RESTART_LIMIT.with(std::cell::Cell::get)
}

/// Folds consecutive operations into as few as `LiveEdit::absorb_operation` allows, each
/// numbered as the last it absorbed and kept under the relay limit.
fn fold<C: LiveEdit>(ops: Vec<SessionOp>) -> Vec<SessionOp> {
    let mut folded = Vec::new();
    let mut current: Option<(SessionOp, C::Op, usize)> = None;
    let finish = |current: (SessionOp, C::Op, usize)| SessionOp {
        payload: Payload::Inline(C::encode_operation(&current.1)),
        ..current.0
    };
    for op in ops {
        let decoded = match &op.payload {
            Payload::Inline(bytes) => C::decode_operation(bytes).ok(),
            Payload::Part(_) => None,
        };
        let Some(operation) = decoded else {
            folded.extend(current.take().map(finish));
            folded.push(op);
            continue;
        };
        let size = op.payload.length() as usize;
        let leftover = match &mut current {
            Some((last, into, bound)) if *bound + size <= LARGEST_RELAYED_OPERATION => {
                let leftover = C::absorb_operation(into, operation);
                if leftover.is_none() {
                    *bound += size;
                    last.id = op.id;
                    last.sequence = op.sequence;
                }
                leftover
            }
            _ => Some(operation),
        };
        if let Some(operation) = leftover {
            folded.extend(current.replace((op, operation, size)).map(finish));
        }
    }
    folded.extend(current.map(finish));
    folded
}

/// The content of `bytes` (the type's default for none) with `journal` replayed on top, for a
/// reader outside the session.
pub(crate) fn replay<C: LiveEdit + Default>(
    bytes: Option<&[u8]>,
    journal: &Journal,
) -> Result<Vec<u8>, String> {
    let mut content = match bytes {
        Some(bytes) => C::decode(bytes).map_err(|error| error.to_string())?,
        None => C::default(),
    };
    if !journal.base.state.is_empty() {
        content
            .adopt_session_state(&journal.base.state)
            .map_err(|error| error.to_string())?;
    }
    for entry in &journal.entries {
        let Some(Wire::Session(SessionMessage::Accepted { op })) = Wire::decode(&entry.payload)
        else {
            return Err("a journal entry this build cannot read".into());
        };
        let Payload::Inline(bytes) = &op.payload else {
            // Only a replica reads an operation's part; the owner commits once it sequences one.
            return Err("a journal entry names an operation by its part".into());
        };
        let operation = C::decode_operation(bytes).map_err(|error| error.to_string())?;
        content.apply(&operation);
    }
    Ok(content.encode())
}

/// The parts the operations a message carries are read from.
fn named_parts(wire: &Wire) -> Vec<PartRef> {
    let Wire::Session(message) = wire else {
        return Vec::new();
    };
    match message {
        SessionMessage::Submit { payload, .. } => payload.part().into_iter().collect(),
        SessionMessage::Accepted { op } => op.payload.part().into_iter().collect(),
        SessionMessage::Snapshot { ops, .. } => {
            ops.iter().filter_map(|op| op.payload.part()).collect()
        }
        _ => Vec::new(),
    }
}

/// The bytes an operation carried by its part adds to what the journal holds.
fn parted(op: &SessionOp) -> usize {
    op.payload.part().map_or(0, |part| {
        usize::try_from(part.length()).unwrap_or(usize::MAX)
    })
}
