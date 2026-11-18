use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use be_block_commit::{CommitId, MergeResult};
use be_block_content::{
    Landed, LiveEdit, Merge, Settling,
    history::{GestureId, GestureKind, Stepped},
};
use be_block_protocol::{ClientId, SessionState};
use be_block_session::Membership;
use be_block_session::{OpId, Payload, Resume, SessionMessage, SessionOp, Submission};
use be_block_store::{Hash, PartRef};
use be_sdk_type_api::{
    Change, EditorId, Endpoint, FileChange, Journal, JournalBase, KeptSubmission, Merging,
    Recipients, ReplicaId, SessionTag, Status, UndoState, Unjournaled, Written,
};
use uuid::Uuid;

use crate::holds::{Holds, This, Took};
use crate::lacking::{Carried, Lacking};
use crate::{
    FromFileFn, Recorder,
    editor::{EditorOperation, FromEditor, Gesture, PeerMessage, PeerPresence, Snapshot, ToEditor},
    effect::{Answer, Effect, HeadEvent, RequestId, Token},
    wire::{self, Entry, Submitted},
};

/// The lease holder commits once its session has gone this long with no edits: a natural
/// pause, which is the state retention prefers to keep.
pub const QUIET_INTERVAL: u64 = 30_000;

/// How long a replica waits before it puts again the part of an edit whose put failed.
const PUT_RETRY_INTERVAL: u64 = 5_000;

/// The longest a replica waits between puts of an edit's part, which double from
/// `PUT_RETRY_INTERVAL`.
const PUT_RETRY_CEILING: u64 = 60_000;

/// How long the first wait is before a part that could not be read is read again; it doubles up
/// to `READ_RETRY_CEILING`. The order waits behind it: every replica applies the entry, so none
/// skips it for a read that failed here.
const READ_RETRY_CEILING: u64 = 60_000;
const READ_RETRY_INTERVAL: u64 = 1_000;

/// The longest part an entry may name: one longer is passed over by every replica alike, unread.
const LARGEST_PART: u64 = 64 * LARGEST_RELAYED_OPERATION as u64;

/// The lease holder commits at most this long after the first edit since its last commit, while
/// edits go on, which bounds what a restore point misses and what a reader replays.
pub const COMMIT_INTERVAL: u64 = 5 * 60_000;

/// The lease holder commits once the order since its last commit passes this, so replaying a
/// journal stays well under a second.
pub const JOURNAL_BYTES: usize = 1024 * 1024;

/// The holder restarts its session at a commit once the session state passes this and outgrows
/// the visible content: tombstones and positions kept for concurrent edits, which a reload from
/// the commit drops. It waits for a commit it makes alone, so nobody's edits need carrying over,
/// until the state passes `RESTART_CEILING` times this.
pub const RESTART_STATE_BYTES: usize = 1024 * 1024;

/// How far past `RESTART_STATE_BYTES` session state grows before a holder with others in the
/// session restarts anyway.
pub const RESTART_CEILING: usize = 4;

/// How long a replica asked to close waits for the order to bring back what it submitted before
/// it seals and goes.
const CLOSING_PATIENCE: u64 = 5_000;

const JOURNAL_LIMIT: usize = 1024;
/// An operation whose bytes pass this goes by the part they were put as (`Payload::Part`).
pub(crate) const LARGEST_RELAYED_OPERATION: usize = 1024 * 1024;
/// How many operations a replica keeps of the order since its last seal, to apply again on top of
/// a reload: past this a reload applies what it kept.
const LOG_LIMIT: usize = 65_536;
const DIVERGED: &str = "the session runs a different version of this block's type";
const CLAIMED_ARRIVAL: &str = "an edit may not say when the server took it";
pub(crate) const UNCERTAIN_EDIT: &str = "an edit kept while this device was away may have \
     been sent already and was not sent again: the session no longer records whether it landed";
pub(crate) const STALE_EDIT: &str = "an edit kept while this device was away was not sent: the session \
     started over from its saved content meanwhile, so what the edit changed is gone";

/// Whether `operation` already holds an arrival stamp (`LiveEdit::arrival`).
fn claims_arrival<C: LiveEdit>(operation: &mut C::Op) -> bool {
    C::arrival(operation).is_some_and(|slot| slot.is_some())
}

pub(crate) struct Opened {
    pub(crate) replica: ReplicaId,
    pub(crate) content_type: Uuid,
    pub(crate) organization: Uuid,
    pub(crate) client: ClientId,
    pub(crate) tag: SessionTag,
    pub(crate) origin: u64,
    pub(crate) taken_before: u64,
    pub(crate) doubtful_through: u64,
    pub(crate) kept_restart: u64,
    pub(crate) kept_through: u64,
    pub(crate) kept_submitted: u64,
    pub(crate) kept_submissions: Vec<KeptSubmission>,
    pub(crate) head: Option<CommitId>,
    pub(crate) session: SessionState,
    pub(crate) journal: Option<Journal>,
    pub(crate) sealed_head: Option<be_sdk_type_api::SealedHead>,
    pub(crate) lost_edits: bool,
    pub(crate) journal_kept: bool,
    pub(crate) commits_kept: be_sdk_type_api::KeptCommits,
    pub(crate) refused_commits: Vec<CommitId>,
    pub(crate) server_head: Option<CommitId>,
    pub(crate) epoch: u64,
    pub(crate) account: Uuid,
    pub(crate) from_file: Option<FromFileFn>,
    /// The type's merge by parts (`MergeParts`), as a `PartsMerge<C>` for its content, if it
    /// merges by parts.
    pub(crate) parts_merge: Option<std::rc::Rc<dyn std::any::Any>>,
    /// The type's fold of its signed log (`LogFold`), as a `Fold<C>`, if it has a log.
    pub(crate) fold: Option<std::rc::Rc<dyn std::any::Any>>,
    pub(crate) now: u64,
}

/// How a type folds an entry of its signed log into what it shows (`LogFold::fold`).
pub(crate) type Fold<C> = fn(&mut C, &be_block_content::LogItem);

/// The entries of its log a replica reads as it opens: enough to describe a short log whole,
/// and to follow what is appended after.
const LOG_OPENED: u32 = 256;

pub(crate) trait AnyReplica {
    fn start(&mut self, out: &mut Vec<Effect>);

    fn input(&mut self, now: u64, input: Input, out: &mut Vec<Effect>);

    fn tick(&mut self, now: u64, out: &mut Vec<Effect>);

    fn wake_at(&self) -> Option<u64>;

    /// The soonest of `wake_at`'s timers that keep the block in step with the server: not a
    /// read's retry.
    fn in_step_at(&self) -> Option<u64>;

    fn released(&self) -> bool;

    /// What the server lacks of the edits made here, for the block's next replica.
    fn carry(&mut self) -> Carried;

    fn adopt(&mut self, carried: Carried);
}

/// How the host answered a submission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Outcome {
    Held,
    Kept,
    Failed(String),
    /// Refused for want of edit access: it never reaches the order.
    Refused(String),
}

pub(crate) enum Input {
    Message {
        from: Endpoint,
        bytes: Vec<u8>,
    },
    Attach {
        editor: EditorId,
        editable: bool,
        /// How many of the editor's operations the opened content holds (`attach`).
        acknowledged: u64,
    },
    /// What the host kept of an editor's edits was given again (`resent`).
    Resent {
        editor: EditorId,
    },
    Detach {
        editor: EditorId,
    },
    Head(HeadEvent),
    Lease {
        session: SessionState,
    },
    /// A seal moved the head to `head`, holding the order through `through` in the run
    /// `restarts`.
    Sealed {
        head: CommitId,
        through: u64,
        restarts: u64,
    },
    /// The order numbered these, in order.
    Sequenced(Vec<SessionOp>),
    Read {
        token: Token,
        result: Result<be_block_content::Stored, String>,
    },
    PartRead {
        token: Token,
        result: Result<Vec<u8>, String>,
    },
    PartPut {
        token: Token,
        result: Result<be_block_content::PartRef, String>,
    },
    /// Where an entry appended to the block's signed log went (`host.append`).
    Appended {
        token: Token,
        result: Result<Option<(u64, i64)>, String>,
    },
    /// A run of the block's signed log read (`host.read-log`): its length and the entries.
    LogRead {
        token: Token,
        result: Result<(u64, Vec<be_block_content::LogItem>), String>,
    },
    /// Entries appended to the block's signed log (`replica.logged`).
    Logged(Vec<be_block_content::LogItem>),
    Resumed {
        token: Token,
        result: Result<Resume, String>,
    },
    /// The host's answer to the submission `counter`.
    Submitted {
        counter: u64,
        outcome: Outcome,
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
        message: Option<SessionMessage>,
    },
    /// The next entry of the order, held back while the parts it names are read.
    Ordered(SessionOp),
    Seal {
        sealing: Sealing,
        reply: Option<RequestId>,
    },
    Reconcile,
    Replace {
        content: C,
        reply: Option<RequestId>,
    },
    /// A part this replica put for one of its operations was answered: what waited behind it
    /// is submitted.
    Unblocked,
    Release,
}

impl<C> Job<C> {
    /// Whether the job may run while a seal is on its way: the order goes on meanwhile, and so
    /// does what editors do.
    fn passes_a_seal(&self) -> bool {
        matches!(
            self,
            Job::Ordered(_)
                | Job::Heard { .. }
                | Job::Unblocked
                | Job::Input(
                    Input::Message {
                        from: Endpoint::Editor(_),
                        ..
                    } | Input::Attach { .. }
                        | Input::Resent { .. }
                        | Input::Detach { .. }
                        | Input::Content { .. }
                        | Input::Lease { .. }
                        | Input::Change {
                            change: Change::Child(_) | Change::Undo | Change::Redo,
                            ..
                        }
                )
        )
    }
}

/// A part a replica puts: for an editor that asked, or the bytes of one of its own operations
/// too large to submit inline, by their hash.
#[derive(Clone, Copy)]
enum Putting {
    Editor { editor: EditorId, token: u64 },
    Operation(Hash),
}

/// An operation too large to submit inline, as the part its bytes are put as.
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

/// What this replica submitted that has not come back numbered.
struct Sent<C: LiveEdit> {
    id: OpId,
    /// The operation it submitted, none for a reload or a stash to keep.
    operation: Option<C::Op>,
    payload: Payload,
    entry: Entry,
    /// A replacement's request, answered once it lands.
    reply: Option<RequestId>,
}

/// An edit of the tentative layer: made in an open tentative gesture, or one that committed and
/// waits for the rest, shown but not sent (guides/type_modules.md, Tentative gestures).
struct Held<Op> {
    gesture: GestureId,
    /// The serial its message is held under (`Holds`), which numbers it once the layer commits.
    serial: u64,
    operation: Op,
}

/// An operation the order applied since the last seal, kept to apply again on top of a reload.
struct Logged<Op> {
    sequence: u64,
    operation: Op,
    /// This replica made it.
    mine: bool,
}

enum Then {
    Load,
    Reload {
        op: SessionOp,
        commit: CommitId,
        head: bool,
        replace: bool,
        restart: bool,
        state: Vec<u8>,
        through: u64,
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

enum AfterPublish {
    Seal {
        retry: bool,
        reply: Option<RequestId>,
        /// The stash this seal keeps as its second parent.
        kept: Option<CommitId>,
        /// The order it holds, and the session state as of it.
        through: u64,
        state: Vec<u8>,
    },
    /// The holder merged a head moved from elsewhere: the order then reloads it.
    Merged { through: u64 },
}

enum AfterStash<C> {
    Merge {
        value: C,
        expected: CommitId,
        base: Option<CommitId>,
        count: usize,
        through: u64,
    },
    /// What this replica had not seen sealed, kept before its replacement: the replacement is
    /// stashed on top of it.
    Unsaved {
        content: C,
        reply: Option<RequestId>,
    },
    /// The replacement, stashed: the order takes it as a barrier.
    Replacement {
        state: Vec<u8>,
        reply: Option<RequestId>,
    },
    /// What a replacement of another replica would drop here, kept in history.
    Keep,
}

/// A type's merge by parts, over the three contents, reading what it needs with the reader.
pub(crate) type PartsMerge<C> =
    fn(
        C,
        C,
        C,
        be_block_content::PartReader,
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
    /// The head moved to `to` while this holder had work the server lacks: ours is what it
    /// confirmed.
    FastForward { to: CommitId, through: u64 },
    Resume {
        base: Option<CommitId>,
        ours: CommitId,
        theirs: CommitId,
        unsealed: bool,
        ours_content: C,
        through: u64,
    },
}

/// How often a part read of a merge is tried before the merge gives up and reports it.
const MERGE_READ_TRIES: u8 = 3;

/// How long a merge that could not read its parts waits to be tried again, doubling up to the
/// ceiling.
const MERGE_RETRY_INTERVAL: u64 = 5_000;
const MERGE_RETRY_CEILING: u64 = 60_000;

/// How long a holder whose seal failed (its block's key not on this device while offline, say)
/// waits before it seals again, doubling up to `SEAL_RETRY_CEILING`.
const SEAL_RETRY_INTERVAL: u64 = 5_000;
const SEAL_RETRY_CEILING: u64 = 60_000;

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
        then: AfterPublish,
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
}

pub(crate) struct Replica<C: LiveEdit, H> {
    id: ReplicaId,
    content_type: Uuid,
    organization: Uuid,
    client: ClientId,
    tag: SessionTag,
    /// What this replica's submissions name it, kept across its reopenings.
    origin: u64,
    from_file: Option<FromFileFn>,
    parts_merge: Option<PartsMerge<C>>,
    fold: Option<Fold<C>>,
    /// The entries of the block's signed log this replica heard or read, by place, each id once
    /// at its first place, folded onto what it describes.
    signed_log: BTreeMap<u64, be_block_content::LogItem>,
    signed_ids: BTreeMap<Uuid, u64>,
    /// What it describes with a type's log: the content as last described, with the log's
    /// entries before the place it names folded onto it, which the log's next entries fold onto
    /// in turn while the content stays as it is.
    folded: Option<(C, u64)>,
    /// Entries of the log heard or read since it last described.
    log_grew: bool,
    /// Appends an editor asked for, by the token of the call: the editor and its token.
    appends: BTreeMap<Token, (EditorId, u64)>,
    /// Reads of the log, by the token of the call: the editor and its token, or none for the
    /// replica's own as it opens.
    log_reads: BTreeMap<Token, Option<(EditorId, u64)>>,
    state: SessionState,
    /// The order applied so far.
    confirmed: C,
    /// `confirmed` with this replica's own edits not yet numbered on top.
    visible: C,
    /// What it submitted that has not come back numbered, in order.
    pending: VecDeque<Sent<C>>,
    /// Edits waiting to be submitted: the operation, a bound on its size, how many edits it
    /// absorbed, the last local edit it holds, and the submission of an earlier opening that
    /// holds them, which it goes as (`kept_submissions`).
    unsent: VecDeque<(C::Op, usize, u64, u64, Option<KeptSubmission>)>,
    /// The tentative layer: every open tentative gesture's edits, in the order made, shown on
    /// top of `unsent` and never submitted until the last of them ends.
    tentative: Vec<Held<C::Op>>,
    /// `visible` without the layer, while it holds edits, once something needed it: what an
    /// ordinary edit's undo step is recorded against, and what is stashed or read.
    beneath: Option<C>,
    /// Which editor messages the open tentative gestures hold, followed as the host follows them.
    holds: Holds,
    /// The open tentative gestures this replica ended itself (an edit of theirs did not carry
    /// over a restart, a replacement took what they moved): their edits are dropped until their
    /// editors end them.
    voided: BTreeSet<GestureId>,
    /// The held message being taken, and its gesture.
    held_now: Option<(GestureId, u64)>,
    /// The undo being taken cancelled the layer, which is all it does.
    undo_spent: bool,
    in_flight: u64,
    covered: bool,
    /// The last sequence number of the order applied.
    applied: u64,
    /// Entries numbered past `applied + 1`, until those before them arrive.
    ahead: BTreeMap<u64, SessionOp>,
    log: VecDeque<Logged<C::Op>>,
    /// The head, and the sequence of the order it holds, with the session state as of it.
    base: Option<CommitId>,
    sealed: u64,
    sealed_state: Vec<u8>,
    /// Each origin's highest `through` the order holds, and the sequence holding it: carried in
    /// the journal's base and in reloads (`wire::BaseState`), so a replica opened again knows its
    /// own edits after the entries that held them were trimmed.
    origins: BTreeMap<u64, (u64, u64)>,
    /// A seal the holder announced past what this replica applied, taken once it applies that
    /// far.
    heard_seal: Option<(CommitId, u64)>,
    /// Stashes the order kept, which the holder's seals take as second parents, oldest first.
    keep: Vec<CommitId>,
    /// The reloads this holder submitted that have not come back: it seals none meanwhile.
    reloading: Vec<OpId>,
    /// The last barrier applied.
    replaced_at: u64,
    /// The counter of this replica's last submission, from a random start: ids never repeat.
    counter: u64,
    /// What earlier openings submitted of the local edits this one is given again, oldest first
    /// (`opening.kept-submissions`): the edits each holds go as exactly that submission, under
    /// its id, which the server answers as held if the earlier one reached it. Each is let go
    /// once a local edit past it is taken, or the order holds it.
    kept_submissions: VecDeque<KeptSubmission>,
    /// The most local edits (`Submitted::through`) of this replica's origin the order holds.
    own_through: u64,
    /// Own entries the order numbered since the last seal, with what each holds of the local
    /// edits: how many the head holds once a seal holds them.
    own_numbered: VecDeque<(u64, u64)>,
    presence: BTreeMap<(ClientId, Uuid), Vec<u8>>,
    shown: BTreeMap<Uuid, Vec<u8>>,
    presence_changed: bool,
    channel: Vec<PeerMessage>,
    incompatible: bool,
    diverged: bool,
    unreadable: Option<String>,
    history: H,
    editors: BTreeMap<EditorId, bool>,
    acknowledged: BTreeMap<EditorId, u64>,
    /// Editors attached that wait for `resent` before they are sent their snapshot, and
    /// nothing of the content until then.
    holding: BTreeSet<EditorId>,
    /// The hash of the snapshot each editor was told last, encoded, while nothing told it
    /// since changed what it shows: a replica opened again in this one's place (`carry`) does
    /// not tell an editor a snapshot it holds already.
    snapshots_shown: BTreeMap<EditorId, be_block_store::Hash>,
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
    /// When this replica made the first edit no seal holds yet: a holder's pause is timed from
    /// it, not from when the order brought the edit back.
    edited_since: Option<u64>,
    opened_journal: Option<Journal>,
    /// The seal made on the head it opened at, which the server took before the journal was
    /// trimmed (`opening.sealed-head`): taken once the journal is replayed.
    sealed_head: Option<be_sdk_type_api::SealedHead>,
    /// When a replica puts again the part of an operation whose put failed.
    put_retry_at: Option<u64>,
    /// When a merge that could not read its parts is tried again, and how often it failed since
    /// one resolved.
    merge_retry_at: Option<u64>,
    merge_failures: u8,
    /// When a holder whose seals failed seals again, and how many failed in a row.
    seal_retry_at: Option<u64>,
    seal_failures: u8,
    /// The bytes of the order since the last seal.
    ordered_bytes: usize,
    restarted_at: Option<CommitId>,
    lost_edits: bool,
    /// How many local edits, counted across openings, the head holds.
    sealed_taken: u64,
    /// How many local edits (an editor's messages, the host's changes) this replica has taken
    /// since it opened, applied or dropped, which its status reports (`Status::taken`): the
    /// host keeps each one it sent past that count to give again should the replica reopen.
    taken: u64,
    /// How many local edits earlier openings of this replica took that this one is not given
    /// (`opening.taken-before`): `taken` counts on from there.
    taken_before: u64,
    /// The last local edit given again that an earlier opening submitted without hearing it
    /// ordered (`opening.doubtful-through`): shown, in `doubtful` with every local edit made
    /// after it, but neither numbered nor submitted, since the server may hold it already. Only
    /// before the replica joins.
    doubtful_through: u64,
    /// Those edits, in the order made, with the number of the first.
    doubtful: Vec<C::Op>,
    first_doubtful: Option<u64>,
    /// Whether the local edit being taken is doubtful: `edit` keeps what it makes in `doubtful`.
    doubtful_now: bool,
    /// How many times the session started over before what this replica applied
    /// (`status.restarts`, `wire::BaseState::restarts`).
    restarts: u64,
    /// What the host kept across a stop (`opening.kept-restart`, `opening.kept-through`): once
    /// loaded on an order restarted since, the local edits through `stale_through` are dropped.
    kept_restart: u64,
    kept_through: u64,
    stale_through: u64,
    /// The last local edit the host submitted for this origin (`opening.kept-submitted`), and
    /// how far a base that no longer names the origin leaves its edits possibly sent.
    kept_submitted: u64,
    uncertain_through: u64,
    /// Whether a base this replica took was full and did not name its origin: aged out of it.
    aged_out: bool,
    /// Whether the journal opened on holds an entry of this origin: how far the order holds its
    /// edits is known exactly then, whatever the base names.
    own_in_journal: bool,
    /// The first local edit that replaces the content and has not landed yet: none taken since
    /// counts as in the order until it lands or fails.
    replacing_from: Option<u64>,
    /// The last local edit a replacement from elsewhere dropped unsent: a stash or reload counts
    /// it in the order (`entry_through`), so a replica opened again never sends it onto the
    /// content that replaced it.
    replaced_through: u64,
    /// Which opening this is (`Opened::epoch`), said with every status.
    epoch: u64,
    sealing: bool,
    closing: Option<u64>,
    loaded: bool,
    released: bool,
    reported: Reported,
    ranges: Vec<RangeRead>,
    /// Parts this replica asked the host to put, by the token of the call.
    puts: BTreeMap<Token, Putting>,
    /// This replica's operations too large to submit inline, by the hash of their bytes.
    uploads: HashMap<Hash, Upload>,
    /// The bytes of operations sent by their part that this replica put or read, until it has
    /// applied them and nothing pending names them.
    payloads: HashMap<PartRef, Vec<u8>>,
    /// Payload parts being read, by the token of the call.
    reading: BTreeMap<Token, PartRef>,
    /// Reads of parts that failed, how often, and when each is tried again.
    read_failures: HashMap<PartRef, u32>,
    read_retry: HashMap<PartRef, u64>,
    /// How many times in a row a put failed.
    put_failures: u32,
    /// What the server lacks of the edits made here (`ToEditor::Lacking`).
    lacking: Lacking,
    /// The journal opened on is this device's own, numbered offline (`opening.journal-kept`).
    journal_kept: bool,
    /// What commits of the block this device keeps (`opening.commits-kept`).
    commits_kept: be_sdk_type_api::KeptCommits,
    /// The refused commits this device holds (`opening.refused-commits`).
    refused_commits: Vec<CommitId>,
    /// The server's head as this device last heard it, read beside `base` on opening when
    /// commits are kept (`opening.server-head`), until the content's own is loaded.
    server_head: Option<Option<CommitId>>,
    /// The account this device signs in as (`opening.account`): what it added is what a
    /// restart shows as still sending.
    account: Uuid,
    out: Vec<Effect>,
}

impl<C, H> AnyReplica for Replica<C, H>
where
    C: LiveEdit + Merge + Clone + Default,
    H: Recorder<C>,
{
    fn start(&mut self, out: &mut Vec<Effect>) {
        match (self.base, self.server_head) {
            // What the commits kept add over the server's head is read with them.
            (Some(head), Some(server)) => self.read(vec![Some(head), server], Then::Load),
            (Some(head), None) => self.read(vec![Some(head)], Then::Load),
            (None, _) => self.load(C::default()),
        }
        self.run(out);
    }

    fn input(&mut self, now: u64, input: Input, out: &mut Vec<Effect>) {
        self.now = self.now.max(now);
        match input {
            Input::Read { token, result } => self.answer_read(token, result),
            Input::PartRead { token, result } => self.answer_part_read(token, result),
            Input::PartPut { token, result } => self.answer_part_put(token, result),
            Input::Appended { token, result } => {
                if let Some((editor, token)) = self.appends.remove(&token) {
                    self.tell_editor(editor, &ToEditor::Appended { token, result });
                }
            }
            Input::LogRead { token, result } => self.answer_log_read(token, result),
            Input::Logged(items) => self.logged(items),
            Input::Resumed { token, result } => self.answer_resume(token, result),
            Input::Message {
                from: Endpoint::Peer(from),
                bytes,
                ..
            } => self.jobs.push_back(Job::Heard {
                from,
                message: wire::decode(&bytes),
            }),
            Input::Sequenced(ops) => {
                for op in ops {
                    self.jobs.push_back(Job::Ordered(op));
                }
            }
            Input::Submitted { counter, outcome } => self.answer_submitted(counter, outcome),
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
        let due: Vec<PartRef> = self
            .read_retry
            .iter()
            .filter(|(_, at)| self.now >= **at)
            .map(|(part, _)| *part)
            .collect();
        for part in due {
            self.read_retry.remove(&part);
        }
        if self.put_retry_at.is_some_and(|at| self.now >= at) {
            self.put_retry_at = None;
            self.flush();
        }
        if self.merge_retry_at.is_some_and(|at| self.now >= at) {
            self.merge_retry_at = None;
            // The holder reconciles; any other replica leaves that to the holder.
            if self.is_holder() {
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
        self.run(out);
    }

    fn wake_at(&self) -> Option<u64> {
        self.in_step_at()
            .into_iter()
            .chain(self.read_retry.values().copied())
            .min()
    }

    fn in_step_at(&self) -> Option<u64> {
        let seal = match self.sealing {
            true => None,
            false => self.commit_at(),
        };
        seal.into_iter()
            .chain(self.put_retry_at)
            .chain(self.merge_retry_at)
            .chain(self.closing.map(|since| since + CLOSING_PATIENCE))
            .min()
    }

    fn released(&self) -> bool {
        self.released
    }

    fn carry(&mut self) -> Carried {
        Carried {
            snapshots_shown: std::mem::take(&mut self.snapshots_shown),
            ..self.lacking.carry()
        }
    }

    fn adopt(&mut self, mut carried: Carried) {
        self.snapshots_shown = std::mem::take(&mut carried.snapshots_shown);
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
            origin: opened.origin,
            from_file: opened.from_file,
            parts_merge: opened.parts_merge.as_ref().and_then(|merge| {
                let merge = merge.downcast_ref::<PartsMerge<C>>().copied();
                debug_assert!(merge.is_some(), "a merge by parts of another content type");
                merge
            }),
            fold: opened.fold.as_ref().and_then(|fold| {
                let fold = fold.downcast_ref::<Fold<C>>().copied();
                debug_assert!(fold.is_some(), "a fold of another content type");
                fold
            }),
            signed_log: BTreeMap::new(),
            signed_ids: BTreeMap::new(),
            folded: None,
            log_grew: false,
            appends: BTreeMap::new(),
            log_reads: BTreeMap::new(),
            state: opened.session,
            projected: Settling::new(empty.clone(), Vec::new()),
            sealed_state: Vec::new(),
            origins: BTreeMap::new(),
            visible: empty.clone(),
            confirmed: empty,
            pending: VecDeque::new(),
            unsent: VecDeque::new(),
            tentative: Vec::new(),
            beneath: None,
            holds: Holds::default(),
            voided: BTreeSet::new(),
            held_now: None,
            undo_spent: false,
            in_flight: 0,
            covered: true,
            applied: 0,
            ahead: BTreeMap::new(),
            log: VecDeque::new(),
            base: opened.head,
            sealed: 0,
            heard_seal: None,
            keep: Vec::new(),
            reloading: Vec::new(),
            replaced_at: 0,
            // Leaves room for 2^48 submissions after it.
            counter: Uuid::new_v4().as_u64_pair().0 >> 16,
            // In order, each holding the edits after the one before it: one that does not is not
            // as the edits it would hold, and they go anew.
            kept_submissions: opened.kept_submissions.into_iter().fold(
                VecDeque::new(),
                |mut kept, submission| {
                    if kept
                        .back()
                        .is_none_or(|last: &KeptSubmission| last.through < submission.through)
                    {
                        kept.push_back(submission);
                    }
                    kept
                },
            ),
            own_through: 0,
            own_numbered: VecDeque::new(),
            presence: BTreeMap::new(),
            shown: BTreeMap::new(),
            presence_changed: false,
            channel: Vec::new(),
            incompatible: false,
            diverged: false,
            unreadable: None,
            history: H::default(),
            editors: BTreeMap::new(),
            acknowledged: BTreeMap::new(),
            holding: BTreeSet::new(),
            snapshots_shown: BTreeMap::new(),
            landed: Vec::new(),
            resnapshot: false,
            described: false,
            jobs: VecDeque::new(),
            waiting: None,
            tokens: 0,
            now: opened.now,
            owes_since: None,
            last_edit: None,
            edited_since: None,
            opened_journal: opened.journal,
            sealed_head: opened.sealed_head,
            put_retry_at: None,
            merge_retry_at: None,
            merge_failures: 0,
            seal_retry_at: None,
            seal_failures: 0,
            ordered_bytes: 0,
            restarted_at: None,
            lost_edits: opened.lost_edits,
            taken: 0,
            sealed_taken: 0,
            taken_before: opened.taken_before,
            doubtful_through: opened.doubtful_through,
            doubtful: Vec::new(),
            first_doubtful: None,
            doubtful_now: false,
            restarts: 0,
            kept_restart: opened.kept_restart,
            kept_through: opened.kept_through,
            stale_through: 0,
            kept_submitted: opened.kept_submitted,
            uncertain_through: 0,
            aged_out: false,
            own_in_journal: false,
            replacing_from: None,
            replaced_through: 0,
            epoch: opened.epoch,
            sealing: false,
            closing: None,
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
            put_failures: 0,
            lacking: Lacking::default(),
            journal_kept: opened.journal_kept,
            commits_kept: opened.commits_kept,
            refused_commits: opened.refused_commits,
            server_head: (opened.commits_kept == be_sdk_type_api::KeptCommits::Outbox
                && opened.server_head != opened.head)
                .then_some(opened.server_head),
            account: opened.account,
            out: Vec::new(),
        }
    }

    fn run(&mut self, out: &mut Vec<Effect>) {
        loop {
            // A seal that came due as the order brought work back starts now: the host ticks
            // only when its clock moves.
            if !self.sealing
                && self.seal_would_settle()
                && self.commit_at().is_some_and(|at| self.now >= at)
            {
                self.sealing = true;
                self.jobs.push_back(Job::Seal {
                    sealing: Sealing::Due,
                    reply: None,
                });
            }
            self.flush_editors();
            if self.released {
                break;
            }
            if self.closing.is_some() && self.waiting.is_none() && self.settled_to_close() {
                self.closing = None;
                self.jobs.push_front(Job::Release);
                self.jobs.push_front(Job::Seal {
                    sealing: Sealing::Due,
                    reply: None,
                });
            }
            let Some(job) = self.jobs.front() else {
                break;
            };
            if self.waiting.is_some() && !(self.sealing_out() && job.passes_a_seal()) {
                break;
            }
            // The order's next entry waits for the parts it names, and so does all that came
            // after it.
            if let Job::Ordered(op) = job {
                let missing = self.missing_parts(op);
                if !missing.is_empty() {
                    let backing_off = missing
                        .iter()
                        .any(|part| self.read_retry.contains_key(part));
                    if !backing_off {
                        for part in missing {
                            self.read_payload(part);
                        }
                    }
                    break;
                }
            }
            let Some(job) = self.jobs.pop_front() else {
                break;
            };
            self.job(job);
        }
        self.prune_payloads();
        self.tell_lacking();
        self.report();
        out.append(&mut self.out);
    }

    /// Whether a replica asked to close may seal and go: the order brought back what it
    /// submitted, or it waited long enough.
    fn settled_to_close(&self) -> bool {
        let settled = self.unreadable.is_some()
            || !self.loaded
            || self.diverged
            || (self.pending.is_empty() && self.unsent.is_empty() && self.reloading.is_empty());
        settled
            || self
                .closing
                .is_some_and(|since| self.now >= since + CLOSING_PATIENCE)
    }

    /// The parts an entry names that this replica has yet to read, unless one is longer than any
    /// part an entry may name: every replica passes that entry over unread.
    fn missing_parts(&self, op: &SessionOp) -> Vec<PartRef> {
        let Some(Submitted {
            entry: Entry::Operation(Payload::Part(part)),
            ..
        }) = Submitted::decode(&op.payload)
        else {
            return Vec::new();
        };
        if op.sequence <= self.applied
            || op.base < op.floor
            || part.length() > LARGEST_PART
            || self.payloads.contains_key(&part)
        {
            return Vec::new();
        }
        vec![part]
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
            Err(error) => {
                let failures = self.read_failures.entry(part).or_default();
                *failures += 1;
                let failures = *failures;
                if failures == 3 {
                    self.error(format!("an operation's part could not be read: {error}"));
                }
                let wait = (READ_RETRY_INTERVAL << (failures - 1).min(6)).min(READ_RETRY_CEILING);
                self.read_retry.insert(part, self.now + wait);
            }
        }
        if self.reading.is_empty()
            && let Some(Wait::Payloads { .. }) = &self.waiting
            && let Some(Wait::Payloads { loaded }) = self.waiting.take()
        {
            let missing = self.journal_parts();
            match missing.is_empty() {
                true => self.loaded(loaded),
                false => {
                    for part in missing {
                        self.read_payload(part);
                    }
                    self.waiting = Some(Wait::Payloads { loaded });
                }
            }
        }
    }

    /// Drops the bytes of payload parts once nothing waiting or pending names them. Nothing
    /// goes while the replica waits: what it waits for may apply them.
    fn prune_payloads(&mut self) {
        if self.payloads.is_empty() || self.waiting.is_some() {
            return;
        }
        let mut named: HashSet<PartRef> = self.reading.values().copied().collect();
        named.extend(self.pending.iter().filter_map(|sent| sent.payload.part()));
        for job in &self.jobs {
            if let Job::Ordered(op) = job
                && let Some(Submitted {
                    entry: Entry::Operation(Payload::Part(part)),
                    ..
                }) = Submitted::decode(&op.payload)
            {
                named.insert(part);
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
                    "an edit too large to submit inline could not be put: {error}"
                ));
                // Kept, and put again after a while, when it next goes out.
                if !self.unsent.is_empty() {
                    let wait =
                        (PUT_RETRY_INTERVAL << self.put_failures.min(4)).min(PUT_RETRY_CEILING);
                    self.put_failures += 1;
                    self.put_retry_at = Some(self.now + wait);
                }
            }
        }
    }

    /// When the holder commits what it owes: after a quiet spell, at most `COMMIT_INTERVAL`
    /// after it started owing, or at once when the order grew past `JOURNAL_BYTES`.
    fn commit_at(&self) -> Option<u64> {
        // No due seal while a merge cannot read its parts: it would publish on the old base and be
        // rejected at each tick until the merge's timer fires. The work stays in the journal.
        if self.merge_failures > 0 {
            return None;
        }
        let since = self.owes_since?;
        // A seal that failed is tried again on its own timer, not at each pause.
        if let Some(at) = self.seal_retry_at {
            return Some(at);
        }
        if self.ordered_bytes >= JOURNAL_BYTES || !self.keep.is_empty() {
            return Some(since);
        }
        let quiet = self.last_edit.map_or(since, |edited| edited.max(since)) + QUIET_INTERVAL;
        Some(quiet.min(since + COMMIT_INTERVAL))
    }

    fn alone(&self) -> bool {
        self.state
            .clients()
            .iter()
            .all(|participant| *participant == self.client)
    }

    fn is_holder(&self) -> bool {
        self.state.is_holder(self.client)
    }

    /// Notes that the order holds `origin`'s local edits through `through` at `sequence`.
    fn note_origin(&mut self, origin: u64, through: u64, sequence: u64) {
        let noted = self.origins.entry(origin).or_insert((0, 0));
        if through >= noted.0 {
            *noted = (through, sequence);
        }
    }

    /// `content`, the type's session state, with how far the order holds each origin as of
    /// `sequence`: the most recently numbered `BaseState::KEPT_ORIGINS` of them.
    fn base_state(&self, content: Vec<u8>, sequence: u64) -> Vec<u8> {
        let mut origins: Vec<(u64, u64, u64)> = self
            .origins
            .iter()
            .filter(|(_, (_, at))| *at <= sequence)
            .map(|(origin, (through, at))| (*origin, *through, *at))
            .collect();
        origins.sort_by_key(|(_, _, at)| std::cmp::Reverse(*at));
        origins.truncate(crate::wire::BaseState::KEPT_ORIGINS);
        crate::wire::BaseState {
            content,
            origins,
            restarts: self.restarts,
        }
        .encode()
    }

    /// Takes the origins a base or reload carries, this replica's own included, and answers the
    /// type's session state in it; none when it does not decode.
    /// `at_most` is the sequence the state is had at: an origin noted past it was numbered in
    /// an order that started over since, and counts as held there.
    fn adopt_base_state(&mut self, bytes: &[u8], at_most: u64) -> Option<Vec<u8>> {
        let state = crate::wire::BaseState::decode(bytes)?;
        self.restarts = self.restarts.max(state.restarts);
        let full = state.origins.len() >= crate::wire::BaseState::KEPT_ORIGINS;
        self.aged_out = full
            && !state
                .origins
                .iter()
                .any(|(origin, ..)| *origin == self.origin);
        for (origin, through, at) in state.origins {
            self.note_origin(origin, through, at.min(at_most));
            if origin == self.origin {
                self.own_through = self.own_through.max(through);
            }
        }
        Some(state.content)
    }

    /// Starts the journal from this holder's base: its last commit, the sequence it holds, and
    /// the session state as of it.
    fn rebase_journal(&mut self) {
        self.out.push(Effect::Rebase {
            replica: self.id,
            restarts: self.restarts,
            base: JournalBase {
                commit: self.base,
                sequence: self.sealed,
                state: self.base_state(self.sealed_state.clone(), self.sealed),
            },
        });
    }

    /// Counts a local edit as taken as its job runs (`taken`), with the held operations of a
    /// tentative layer that commits as it does, before it (`Holds`). A message a tentative
    /// gesture holds is not a local edit.
    fn take(&mut self, job: &Job<C>) -> (bool, Took) {
        let took = match job {
            Job::Input(Input::Message {
                from: Endpoint::Editor(editor),
                bytes,
                ..
            }) => self
                .holds
                .message(*editor, FromEditor::decode(bytes).as_ref()),
            Job::Input(Input::Change {
                change: Change::Undo,
                ..
            }) => self.holds.undo(),
            Job::Input(Input::Detach { editor }) => self.holds.detach(*editor),
            _ => Took::default(),
        };
        let local = took.this == This::Counted
            && matches!(
                job,
                Job::Input(
                    Input::Message {
                        from: Endpoint::Editor(_),
                        ..
                    } | Input::Change { .. }
                )
            );
        self.taken += took.counted.len() as u64 + u64::from(local);
        (local, took)
    }

    /// The local edit just taken, counted across this replica's openings.
    fn edit_number(&self) -> u64 {
        self.taken_before + self.taken
    }

    fn job(&mut self, job: Job<C>) {
        let base = self.edit_number();
        let (local, took) = self.take(&job);
        self.follow(&job, took, base);
        self.run_job(job, local);
        self.held_now = None;
        self.undo_spent = false;
        // A local edit taken, even one not made, may complete what an earlier opening
        // submitted (`kept_submissions`), which waits for it.
        if local && self.unsent.front().is_some_and(|(.., kept)| kept.is_some()) {
            self.flush();
        }
    }

    /// Does what a job did to the tentative gestures (`take`), before the job itself runs, and
    /// whatever its editor may: their ends, their beginnings, and the layer committing. `base`
    /// is the local edit taken last before it, after which a committing layer's edits number.
    fn follow(&mut self, job: &Job<C>, took: Took, base: u64) {
        if let (
            This::Held(serial),
            Some(gesture),
            Job::Input(Input::Message {
                from: Endpoint::Editor(_),
                ..
            }),
        ) = (took.this, took.gesture, job)
        {
            self.held_now = Some((gesture, serial));
        }
        if matches!(
            job,
            Job::Input(Input::Change {
                change: Change::Undo,
                ..
            })
        ) && took.undone
        {
            // Undo while the layer is held cancels all of it, as Escape would, and is spent.
            // The open gestures stay open, voided, until their editors end them (`Holds::undo`).
            let _ = self.history.undo(&self.visible);
            for gesture in self.holds.open() {
                self.history.begin(*gesture, GestureKind::Tentative);
            }
            self.undo_spent = true;
            for gesture in took.voided {
                if self.voided.insert(gesture) {
                    self.gesture_ended(gesture);
                }
            }
            if !std::mem::take(&mut self.tentative).is_empty() {
                self.rebuild_visible();
            }
            return;
        }
        let mut dropped = false;
        for (gesture, committed) in &took.ended {
            self.voided.remove(gesture);
            match committed {
                true => self.history.commit(*gesture),
                false => {
                    // A tentative gesture's edits were never written: they leave the layer.
                    let _ = self.history.cancel(&self.visible, *gesture);
                    let before = self.tentative.len();
                    self.tentative.retain(|held| held.gesture != *gesture);
                    dropped |= self.tentative.len() != before;
                }
            }
        }
        if took.settled {
            self.commit_layer(base, &took.counted, dropped);
        } else if dropped {
            self.rebuild_visible();
        }
        if let Some(gesture) = took.opened {
            // An undo group open under its name commits first.
            self.history.commit(gesture);
            self.history.begin(gesture, GestureKind::Tentative);
        }
    }

    /// The last open tentative gesture ended: the edits of those that committed leave the
    /// layer, in the order made, for the end of the ordinary path, numbered after `base` in
    /// the order of `counted`. They stay where they show, so are not applied again unless
    /// some cannot go (the order holds them already, or they were kept across a stop that
    /// outdated them).
    fn commit_layer(&mut self, base: u64, counted: &[u64], mut rebuild: bool) {
        let layer = std::mem::take(&mut self.tentative);
        self.beneath = None;
        let mut queued = false;
        for Held {
            serial, operation, ..
        } in layer
        {
            let rank = counted.iter().position(|counted| *counted == serial);
            debug_assert!(
                rank.is_some(),
                "a held edit its layer's commit does not count"
            );
            let Some(rank) = rank else {
                rebuild = true;
                continue;
            };
            let number = base + 1 + rank as u64;
            if number <= self.own_through {
                rebuild = true;
                continue;
            }
            if number <= self.stale_through || number <= self.uncertain_through {
                let why = match number <= self.stale_through {
                    true => STALE_EDIT,
                    false => UNCERTAIN_EDIT,
                };
                self.lacking.add_dropped(C::encode_operation(&operation));
                self.error(why);
                rebuild = true;
                continue;
            }
            if number <= self.doubtful_through || self.first_doubtful.is_some() {
                self.first_doubtful.get_or_insert(number);
                self.doubtful.push(operation);
                continue;
            }
            let announcements = self.visible.announcements(&operation);
            self.queue(operation, number, announcements);
            queued = true;
        }
        if rebuild {
            self.rebuild_visible();
        }
        if queued {
            self.edited_since.get_or_insert(self.now);
            self.last_edit = Some(self.now);
            self.flush();
        }
    }

    /// An edit made in the open tentative gesture `gesture`, its message held under `serial`:
    /// shown on top, told to the other editors, and kept in the layer.
    fn hold(
        &mut self,
        gesture: GestureId,
        serial: u64,
        mut operation: C::Op,
        origin: Option<EditorId>,
        record: bool,
    ) -> bool {
        if self.diverged {
            self.error(DIVERGED);
            return false;
        }
        if claims_arrival::<C>(&mut operation) {
            self.error(CLAIMED_ARRIVAL);
            self.reproject();
            return true;
        }
        // Its gesture ended here: what it moved is gone, so the edit applies as nothing. Its
        // editor, which showed it before hearing, is told what holds now.
        if self.voided.contains(&gesture) {
            return false;
        }
        if record {
            self.history.record(
                &self.visible,
                &operation,
                origin,
                Some(gesture.id),
                self.now,
            );
        }
        if self.tentative.is_empty() {
            self.beneath = Some(self.visible.clone());
        }
        self.visible.apply(&operation);
        let unsettled = self.unsettled() + 1;
        self.journal(
            Journaled::Edited {
                operation: operation.clone(),
                unsettled,
            },
            origin,
        );
        self.tentative.push(Held {
            gesture,
            serial,
            operation,
        });
        true
    }

    /// Ends the open tentative gesture `gesture` here: its edits could not be kept. It stays
    /// open, dropping what it edits, until its editor, told, ends it. A gesture that committed
    /// and waits for the rest only loses its edits.
    fn void(&mut self, gesture: GestureId) {
        self.history.void(gesture);
        self.tentative.retain(|held| held.gesture != gesture);
        if self.holds.open().contains(&gesture) && self.voided.insert(gesture) {
            self.gesture_ended(gesture);
        }
    }

    /// Drops every undo step: the content was replaced. The open tentative gestures stay open,
    /// to end as their editors end them, but the layer is gone with what it moved.
    fn clear_history(&mut self) {
        self.history.clear();
        let open = self.holds.open().to_vec();
        for gesture in &open {
            self.history.begin(*gesture, GestureKind::Tentative);
        }
        let held = !std::mem::take(&mut self.tentative).is_empty();
        for gesture in open {
            if self.voided.insert(gesture) {
                self.gesture_ended(gesture);
            }
        }
        if held {
            self.rebuild_visible();
        }
    }

    /// Tells the editor of `gesture` this replica ended it, keeping none of its edits.
    fn gesture_ended(&mut self, gesture: GestureId) {
        let Some(editor) = gesture.editor else {
            return;
        };
        if self.editors.contains_key(&editor) {
            self.tell_editor(
                editor,
                &ToEditor::GestureEnded {
                    id: gesture.id,
                    cancelled: true,
                },
            );
        }
    }

    /// The content without the tentative layer: what is stashed, read or described, and what
    /// an ordinary edit made beneath the layer is recorded against.
    fn unheld(&mut self) -> &C {
        if self.tentative.is_empty() {
            return &self.visible;
        }
        if self.beneath.is_none() {
            self.beneath = Some(self.layer_free());
        }
        self.beneath.as_ref().unwrap_or(&self.visible)
    }

    /// `confirmed` with this replica's own edits on top, but not the tentative layer.
    fn layer_free(&self) -> C {
        let mut content = self.confirmed.clone();
        for sent in &self.pending {
            if let Some(operation) = &sent.operation {
                content.apply(operation);
            }
        }
        for (operation, ..) in &self.unsent {
            content.apply(operation);
        }
        for operation in &self.doubtful {
            content.apply(operation);
        }
        content
    }

    fn run_job(&mut self, job: Job<C>, local: bool) {
        if self.unreadable.is_some() {
            return self.job_while_unreadable(job);
        }
        // Given again after a reopening, and the order holds it already: it is not made twice.
        if local && self.edit_number() <= self.own_through {
            return self.job_already_ordered(job);
        }
        if local && self.edit_number() <= self.stale_through {
            return self.job_stale(job, STALE_EDIT);
        }
        if local && self.edit_number() <= self.uncertain_through {
            return self.job_stale(job, UNCERTAIN_EDIT);
        }
        // What follows a doubtful edit waits behind it: numbered here, it would be sealed before
        // the edit it was made on, and given again with it once the replica joins.
        self.doubtful_now =
            local && (self.edit_number() <= self.doubtful_through || self.first_doubtful.is_some());
        match job {
            Job::Input(input) => self.handle(input),
            Job::Heard { from, message, .. } => self.heard(from, message),
            Job::Ordered(op) => self.ordered(op),
            Job::Seal { sealing, reply } => self.seal(sealing, reply),
            Job::Reconcile => self.reconcile(),
            Job::Replace { content, reply } => self.replace(content, reply),
            Job::Unblocked => self.flush(),
            Job::Release => self.release(),
        }
        self.doubtful_now = false;
    }

    /// A local edit kept across a stop and made before a restart of the order since: what it
    /// was made on is gone, so it is dropped, its editor told it was not sent, and a change
    /// answered as not made.
    fn job_stale(&mut self, job: Job<C>, why: &str) {
        match job {
            Job::Input(Input::Message {
                from: Endpoint::Editor(editor),
                bytes,
                ..
            }) => match FromEditor::decode(&bytes) {
                Some(FromEditor::Operate { operation, .. }) => {
                    *self.acknowledged.entry(editor).or_default() += 1;
                    self.lacking.add_dropped(operation);
                    self.error(why);
                }
                Some(FromEditor::Gesture { .. } | FromEditor::Break | FromEditor::Replace(_))
                | None => {}
                Some(message) => self.handle_editor(editor, message),
            },
            Job::Input(Input::Change { id, .. }) => self.reply(id, Answer::Changed(false)),
            _ => {}
        }
    }

    /// A local edit the order already holds, given again: its editor counts it, a change is
    /// answered, and nothing is applied.
    fn job_already_ordered(&mut self, job: Job<C>) {
        match job {
            Job::Input(Input::Message {
                from: Endpoint::Editor(editor),
                bytes,
                ..
            }) => match FromEditor::decode(&bytes) {
                Some(FromEditor::Operate { .. }) => {
                    *self.acknowledged.entry(editor).or_default() += 1;
                }
                Some(FromEditor::Gesture { .. } | FromEditor::Break | FromEditor::Replace(_))
                | None => {}
                Some(message) => self.handle_editor(editor, message),
            },
            Job::Input(Input::Change { id, .. }) => self.reply(id, Answer::Changed(true)),
            _ => {}
        }
    }

    fn job_while_unreadable(&mut self, job: Job<C>) {
        match job {
            Job::Input(Input::Close) | Job::Release => self.release(),
            // Editors still come and go, and each is told the content will not open
            // (`snapshot_to`).
            Job::Input(
                input @ (Input::Attach { .. } | Input::Resent { .. } | Input::Detach { .. }),
            ) if !self.released => self.handle(input),
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
        self.replace_settled();
    }

    fn release(&mut self) {
        if self.released {
            return;
        }
        if self.loaded && self.is_holder() && self.alone() && self.is_clean() {
            // The last one out restarts the session for free: whoever opens next loads the
            // commit afresh and the order starts over.
            // It counts as a restart, and keeps how far the order holds each origin's edits.
            self.restarts += 1;
            let state = self.base_state(Vec::new(), self.sealed);
            self.out.push(Effect::Rebase {
                replica: self.id,
                restarts: self.restarts,
                base: JournalBase {
                    commit: self.base,
                    sequence: 0,
                    state,
                },
            });
        }
        self.out.push(Effect::Release { replica: self.id });
        self.released = true;
        for sent in std::mem::take(&mut self.pending) {
            if let Some(id) = sent.reply {
                self.reply(id, Answer::Changed(false));
            }
        }
        for job in std::mem::take(&mut self.jobs) {
            self.take(&job);
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
            Input::Attach {
                editor,
                editable,
                acknowledged,
            } => self.attach(editor, editable, acknowledged),
            Input::Resent { editor } => self.resent(editor),
            Input::Detach { editor } => {
                self.editors.remove(&editor);
                self.acknowledged.remove(&editor);
                self.holding.remove(&editor);
                self.snapshots_shown.remove(&editor);
                self.lacking.unfollow(editor);
            }
            Input::Head(HeadEvent::Moved { head }) => self.moved(head),
            Input::Head(_)
            | Input::Read { .. }
            | Input::PartRead { .. }
            | Input::PartPut { .. }
            | Input::Appended { .. }
            | Input::LogRead { .. }
            | Input::Logged(_)
            | Input::Resumed { .. }
            | Input::Submitted { .. }
            | Input::Sequenced(_)
            | Input::Drained { .. } => {}
            Input::Lease { session } => {
                self.adopt(session);
                // A new lease is a new chance for a failed seal.
                self.seal_failures = 0;
                self.seal_retry_at = None;
                self.flush();
            }
            Input::Sealed {
                head,
                through,
                restarts,
            } => {
                // A seal of another run of the order counts its numbers there: they name other
                // entries here.
                if !self.is_holder() && restarts == self.restarts {
                    self.heard_seal = Some((head, through));
                    self.take_heard_seal();
                }
            }
            Input::Claim => self.claim(),
            Input::Close => {
                self.closing.get_or_insert(self.now);
            }
            Input::Change { id, change } => self.change(id, change),
            Input::Content { id } => match C::LOADS {
                be_block_content::Loads::Whole => {
                    let bytes = self.unheld().encode();
                    self.reply(id, Answer::Content(bytes));
                }
                be_block_content::Loads::FirstPart => {
                    self.read_range(Reader::Content(id), 0, u64::MAX)
                }
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

    fn heard(&mut self, from: ClientId, message: Option<SessionMessage>) {
        let Some(message) = message else {
            self.incompatible = true;
            return;
        };
        match message {
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
        }
    }

    /// Takes a seal the holder announced once this replica applied as far as it holds.
    fn take_heard_seal(&mut self) {
        let Some((head, sequence)) = self.heard_seal else {
            return;
        };
        if sequence > self.applied || sequence < self.sealed {
            if sequence < self.sealed {
                self.heard_seal = None;
            }
            return;
        }
        self.heard_seal = None;
        self.settle_at(head, sequence);
    }

    /// The head holds the order through `sequence`: forgets what it kept to apply on top of a
    /// reload before it, and counts what of this replica's edits the head holds.
    fn settle_at(&mut self, head: CommitId, sequence: u64) {
        self.sealed_taken = self.sealed_taken.max(self.own_through_at(sequence));
        self.base = Some(head);
        if sequence == self.applied {
            self.sealed_state = self.confirmed.session_state();
        }
        self.sealed = sequence;
        while self
            .log
            .front()
            .is_some_and(|logged| logged.sequence <= sequence)
        {
            self.log.pop_front();
        }
        while self
            .own_numbered
            .front()
            .is_some_and(|(at, _)| *at <= sequence)
        {
            self.own_numbered.pop_front();
        }
        self.ordered_bytes = 0;
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
                        first_part: C::LOADS == be_block_content::Loads::FirstPart,
                    });
                    Slot::Waiting(token)
                }
                None => Slot::Ready(C::default()),
            })
            .collect();
        self.waiting = Some(Wait::Reads { slots, then });
        self.finish_reads();
    }

    fn answer_read(&mut self, token: Token, result: Result<be_block_content::Stored, String>) {
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
                    Then::Load => {
                        self.unreadable = Some(error);
                        // Its editors are told it will not open rather than left loading.
                        self.resnapshot = true;
                    }
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
            Then::Load => {
                let loaded = next();
                if self.server_head.take().is_some() {
                    // Opened after a restart with commits still in the outbox: what they add
                    // over the server's head is still sending, unless an earlier replica's
                    // edits were carried, which say so themselves.
                    let unsent: Vec<Vec<u8>> = loaded
                        .unsent_over(&next(), self.account)
                        .iter()
                        .map(C::encode_operation)
                        .collect();
                    if let Some(tip) = self.base {
                        self.lacking.kept_unsent(unsent, tip);
                    }
                }
                self.load(loaded)
            }
            Then::Reload {
                op,
                commit,
                head,
                replace,
                restart,
                state,
                through,
            } => self.reloaded(op, commit, head, replace, restart, state, through, next()),
            Then::FastForwardMerge { to } => {
                let ancestor = next();
                let theirs = next();
                let ours = self.confirmed.clone();
                let through = self.applied;
                self.merging(
                    MergeThen::FastForward { to, through },
                    ancestor,
                    ours,
                    theirs,
                );
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
                let through = self.applied;
                self.merging(
                    MergeThen::Resume {
                        base,
                        ours,
                        theirs,
                        unsealed,
                        ours_content: ours_content.clone(),
                        through,
                    },
                    ancestor,
                    ours_content,
                    theirs_content,
                );
            }
        }
    }

    fn merging(&mut self, then: MergeThen<C>, base: C, ours: C, theirs: C) {
        let Some(merge) = self.parts_merge else {
            let result = C::merge3(&base, &ours, &theirs);
            return self.merged(then, result);
        };
        let reads = std::rc::Rc::new(std::cell::RefCell::new(MergeReads::default()));
        let reader = {
            let reads = std::rc::Rc::clone(&reads);
            be_block_content::PartReader::new(move |part, offset, length| {
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

    /// A seal that failed leaves the work unsealed, journaled, and is tried again on a timer that
    /// doubles up to a ceiling: until then the replica owes no seal (`seal_would_settle`), or a
    /// host that seals what is owed on every frame would fail it on every frame. Once the time
    /// passes, `seal_retry_at` stays set until a seal settles or fails again: `commit_at` makes
    /// the retry due, and `sealing` keeps a second one from starting while it is on its way.
    fn seal_failed(&mut self) {
        let wait = (SEAL_RETRY_INTERVAL << self.seal_failures.min(4)).min(SEAL_RETRY_CEILING);
        self.seal_failures = self.seal_failures.saturating_add(1);
        self.seal_retry_at = Some(self.now + wait);
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

    /// What a finished merge goes on to do.
    fn merged(&mut self, then: MergeThen<C>, result: MergeResult<C>) {
        match then {
            MergeThen::FastForward { to, through } => match result {
                MergeResult::Clean(value) => {
                    let content = self.written(&value);
                    self.publish(Some(to), None, content, AfterPublish::Merged { through });
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
                            through,
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
                through,
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
                                through,
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
                            AfterPublish::Merged { through },
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
        then: AfterPublish,
    ) {
        let token = self.token();
        // A seal holds the order through what it applied when it began, and the journal starts
        // from it with the session state as of then once it wins the head.
        let (sealed_through, journal_state) = match &then {
            AfterPublish::Seal { through, state, .. } => (
                Some(*through),
                Some(self.base_state(state.clone(), *through)),
            ),
            AfterPublish::Merged { .. } => (None, None),
        };
        self.out.push(Effect::Publish {
            replica: self.id,
            token,
            expected,
            merging,
            content,
            sealed_through,
            journal_state,
            restarts: self.restarts,
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
        // A seal of another run of the order counts its sequences there: a head to merge.
        let resume = match resume {
            Resume::Sealed {
                commit, restarts, ..
            } if restarts != self.restarts => Resume::FastForward { to: commit },
            resume => resume,
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
            Resume::Sealed {
                commit, through, ..
            } => {
                self.heard_seal = Some((commit, through));
                self.take_heard_seal();
            }

            // Nothing unsealed: the order takes the head as it is, applying again what it
            // numbered since the seal.
            Resume::FastForward { to } if !unsealed => {
                self.submit_reload(to, true, false, Vec::new(), None);
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
        self.answer_wait(event);
        self.replace_settled();
    }

    fn answer_wait(&mut self, event: HeadEvent) {
        match (self.waiting.take(), event) {
            (Some(Wait::Publish { then, .. }), HeadEvent::Published { head, held, .. }) => {
                self.published(then, head, held);
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
                        self.seal_failed();
                        reply.map(|id| (id, Answer::Sealed))
                    }
                    Some(Wait::Stash {
                        then:
                            AfterStash::Unsaved { reply, .. } | AfterStash::Replacement { reply, .. },
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

    fn published(&mut self, then: AfterPublish, head: CommitId, held: bool) {
        match then {
            AfterPublish::Seal {
                reply,
                kept,
                through,
                state,
                ..
            } => {
                self.sealing = false;
                self.seal_failures = 0;
                self.seal_retry_at = None;
                if let Some(kept) = kept {
                    self.keep.retain(|commit| *commit != kept);
                }
                self.settle_at(head, through);
                self.sealed_state = state;
                self.lacking.sealed(through, held, head);
                // What came after the seal still owes one, timed from now.
                self.owes_since = (self.applied > through).then_some(self.now);
                if self.unsent.is_empty() && self.pending.is_empty() {
                    self.edited_since = None;
                }
                // The journal starts from the seal: the publish carried its base, which the
                // server took as the head moved and every replica hears (`Input::Sealed`).
                if let Some(id) = reply {
                    self.reply(id, Answer::Sealed);
                }
                if self.restart_due() {
                    self.submit_reload(head, true, true, Vec::new(), None);
                }
            }
            AfterPublish::Merged { through, .. } => {
                self.submit_reload_based(head, true, false, Vec::new(), None, through);
            }
        }
    }

    /// The most of this replica's local edits the order holds through `sequence`.
    fn own_through_at(&self, sequence: u64) -> u64 {
        self.own_numbered
            .iter()
            .filter(|(at, _)| *at <= sequence)
            .map(|(_, through)| *through)
            .max()
            .unwrap_or(0)
    }

    fn rejected(&mut self, then: AfterPublish, _head: Option<CommitId>) {
        match then {
            AfterPublish::Seal {
                retry: true, reply, ..
            } if self.merge_failures == 0 => {
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
            // The head moved again under the merge: merge once more.
            AfterPublish::Merged { .. } => self.jobs.push_front(Job::Reconcile),
        }
    }

    fn stashed(&mut self, then: AfterStash<C>, commit: CommitId) {
        match then {
            AfterStash::Merge {
                value,
                expected,
                base,
                count,
                through,
            } => {
                self.conflict(commit, expected, count);
                let content = self.written(&value);
                self.publish(
                    Some(expected),
                    Some(Merging { base, ours: commit }),
                    content,
                    AfterPublish::Merged { through },
                );
            }
            AfterStash::Unsaved { content, reply } => {
                let state = content.session_state();
                let written = self.written(&content);
                self.stash(
                    Some(commit),
                    written,
                    AfterStash::Replacement { state, reply },
                );
            }
            AfterStash::Replacement { state, reply } => {
                self.submit_reload(commit, false, true, state, reply);
            }
            AfterStash::Keep => {
                self.submit_entry(Entry::Keep { commit }, false, None);
            }
        }
    }

    /// Takes the content just read once the parts its journal's operations name are read too.
    fn load(&mut self, content: C) {
        let missing = self.journal_parts();
        if missing.is_empty() {
            return self.loaded(content);
        }
        for part in missing {
            self.read_payload(part);
        }
        self.waiting = Some(Wait::Payloads { loaded: content });
    }

    /// The parts the journal opened on names that are not read yet.
    fn journal_parts(&self) -> Vec<PartRef> {
        self.opened_journal
            .iter()
            .filter(|journal| journal.base.commit == self.base)
            .flat_map(|journal| &journal.entries)
            .filter(|entry| entry.base >= entry.floor)
            .filter_map(|entry| match Submitted::decode(&entry.payload) {
                Some(Submitted {
                    entry: Entry::Operation(Payload::Part(part)),
                    ..
                }) if part.length() <= LARGEST_PART => Some(part),
                _ => None,
            })
            .filter(|part| !self.payloads.contains_key(part) && !self.read_retry.contains_key(part))
            .collect()
    }

    fn loaded(&mut self, content: C) {
        self.confirmed = content;
        self.sealed_state = self.confirmed.session_state();
        let journal = self
            .opened_journal
            .take()
            .filter(|journal| journal.base.commit == self.base);
        let sealed_head = self.sealed_head.take();
        if let Some(journal) = journal {
            match self.adopt_base_state(&journal.base.state, journal.base.sequence) {
                Some(state) if !state.is_empty() => {
                    if self.confirmed.adopt_session_state(&state).is_err() {
                        self.incompatible = true;
                    }
                    self.sealed_state = state;
                }
                Some(_) => {}
                None => self.incompatible = true,
            }
            self.applied = journal.base.sequence;
            self.sealed = journal.base.sequence;
            for entry in journal.entries {
                if entry.sequence <= self.applied {
                    self.passed_over(
                        OpId {
                            origin: entry.origin,
                            counter: entry.counter,
                        },
                        entry.sequence,
                    );
                    continue;
                }
                let op = SessionOp {
                    id: OpId {
                        origin: entry.origin,
                        counter: entry.counter,
                    },
                    sequence: entry.sequence,
                    base: entry.base,
                    floor: entry.floor,
                    received: entry.received,
                    payload: entry.payload,
                };
                self.replay_entry(op);
                if let Some(sealed) = sealed_head.as_ref()
                    && sealed.through == self.applied
                {
                    // What the seal holds replayed onto the commit it was made on: the seal is
                    // the head from here on.
                    self.settle_at(sealed.commit, sealed.through);
                }
            }
        }
        // Local edits kept across a stop were made before a restart the order had since: what
        // they were made on is gone.
        if self.restarts > self.kept_restart {
            self.stale_through = self.kept_through;
            // And what earlier openings submitted went into the order before it restarted.
            self.kept_submissions.clear();
        }
        // The order's base names only its most recent origins: one aged out of it may hold what
        // this origin submitted though no entry says so. Sent again, it could land twice.
        if self.aged_out && !self.own_in_journal && self.kept_submitted > self.own_through {
            self.uncertain_through = self.kept_submitted;
        }
        self.visible = self.confirmed.clone();
        self.projected = Settling::new(self.visible.clone(), Vec::new());
        self.loaded = true;
        self.described = true;
        // A type with a log reads its first entries, which follows the log from now on.
        if self.fold.is_some() && !self.log_reads.values().any(Option::is_none) {
            let token = self.token();
            self.out.push(Effect::ReadLog {
                replica: self.id,
                token,
                from: 0,
                limit: LOG_OPENED,
            });
            self.log_reads.insert(token, None);
        }
        self.check_build();
        // Edits of another build the journal held were lost: the holder starts the journal over
        // from its head, so the order takes this build's.
        if self.lost_edits && self.is_holder() {
            self.rebase_journal();
        }
    }

    /// Applies an entry of the journal opened on, as the order applies it: an operation, or a
    /// stash to keep. A reload in it was made of commits this replica would read; it opens on
    /// the head the journal's holder sealed instead, and the next seal holds it.
    fn replay_entry(&mut self, op: SessionOp) {
        let sequence = op.sequence;
        let mine = op.id.origin == self.origin;
        self.ordered_bytes += ordered_bytes(&op);
        match Submitted::decode(&op.payload) {
            Some(Submitted { through, entry }) => {
                self.note_origin(op.id.origin, through, sequence);
                if mine {
                    self.own_through = self.own_through.max(through);
                    self.own_numbered.push_back((sequence, through));
                    self.own_in_journal = true;
                }
                match entry {
                    Entry::Operation(payload) if op.base >= op.floor => {
                        if let Some(operation) = self.stamped(&payload, op.received) {
                            let bytes = match &payload {
                                Payload::Inline(bytes) => bytes.clone(),
                                Payload::Part(_) => Vec::new(),
                            };
                            if mine {
                                match self.journal_kept {
                                    true => self.lacking.add_replayed(bytes, sequence),
                                    false => self.lacking.journaled(&bytes, Some(sequence)),
                                }
                                // This replica's own, made before it opened again: undo steps
                                // back through it, as through one made here.
                                self.history.record(
                                    &self.confirmed,
                                    &operation,
                                    None,
                                    None,
                                    self.now,
                                );
                            }
                            self.confirmed.apply(&operation);
                            self.log_operation(sequence, operation, mine);
                        }
                    }
                    Entry::Operation(_) => {}
                    Entry::Reload { restart: true, .. } => self.restarts += 1,
                    Entry::Keep { commit }
                    | Entry::Reload {
                        commit,
                        head: false,
                        ..
                    } => {
                        if !self.keep.contains(&commit) {
                            self.keep.push(commit);
                        }
                    }
                    Entry::Reload { .. } => {}
                }
            }
            None => self.incompatible = true,
        }
        self.applied = sequence;
    }

    /// The operation `payload` carries, with its arrival stamped as the order took it: every
    /// replica stamps alike, whatever the sender wrote there.
    fn stamped(&self, payload: &Payload, received: Option<i64>) -> Option<C::Op> {
        let mut operation = self.operation(payload)?;
        if let Some(slot) = C::arrival(&mut operation) {
            *slot = received;
        }
        Some(operation)
    }

    fn log_operation(&mut self, sequence: u64, operation: C::Op, mine: bool) {
        if self.log.len() >= LOG_LIMIT {
            self.log.pop_front();
        }
        self.log.push_back(Logged {
            sequence,
            operation,
            mine,
        });
    }

    /// Whether the session speaks this build: one that does not refuses to edit or seal, and
    /// the server refuses its submissions.
    fn check_build(&mut self) {
        if self
            .state
            .tag()
            .is_some_and(|tag| *tag.as_bytes() != self.tag.0)
        {
            self.incompatible = true;
            self.diverged = true;
        }
    }

    /// An entry numbered at or before what this replica applied, which it passes over: one it
    /// applied comes again, as a replay does. One of its own edits still waiting to be
    /// numbered never does: the order that numbered it is behind the replica's, and the edit
    /// would wait for good, so it says so.
    fn passed_over(&mut self, id: OpId, sequence: u64) {
        if id.origin == self.origin && self.pending.iter().any(|sent| sent.id == id) {
            self.error(format!(
                "its own submission {} came back numbered {sequence}, behind the {} it applied",
                id.counter, self.applied
            ));
        }
    }

    /// The order's next entry: applied in order, held when it arrives early.
    fn ordered(&mut self, op: SessionOp) {
        if op.sequence <= self.applied {
            return self.passed_over(op.id, op.sequence);
        }
        if op.sequence > self.applied + 1 {
            self.ahead.insert(op.sequence, op);
            return;
        }
        self.apply_entry(op);
        while self.waiting.is_none()
            && let Some(next) = self.ahead.remove(&(self.applied + 1))
        {
            self.apply_entry(next);
        }
        // The next edit goes while a seal is out too (`passes_a_seal`): left until the seal
        // lands, it would wait for the next edit to come.
        if self.waiting.is_none() || self.sealing_out() {
            self.flush();
        }
    }

    fn apply_entry(&mut self, op: SessionOp) {
        let mine = op.id.origin == self.origin;
        self.ordered_bytes += ordered_bytes(&op);
        // Another's edit counts from when it arrived; this replica's own from when it was made.
        if !mine {
            self.last_edit = Some(self.now);
        }
        let Some(Submitted { through, entry }) = Submitted::decode(&op.payload) else {
            self.incompatible = true;
            return self.passed(&op, mine);
        };
        self.note_origin(op.id.origin, through, op.sequence);
        if mine {
            if !self.pending.iter().any(|sent| sent.id == op.id) {
                self.heard_own(op.id.counter);
            }
            self.own_through = self.own_through.max(through);
            self.own_numbered.push_back((op.sequence, through));
            self.lacking.numbered(op.id.counter, op.sequence);
        }
        match entry {
            Entry::Operation(payload) => self.apply_operation(op, mine, payload),
            Entry::Reload {
                commit,
                head,
                replace,
                restart,
                state,
            } => {
                if op.base < op.floor && !replace && !restart {
                    return self.passed(&op, mine);
                }
                self.read(
                    vec![Some(commit)],
                    Then::Reload {
                        op,
                        commit,
                        head,
                        replace,
                        restart,
                        state,
                        through,
                    },
                );
            }
            Entry::Keep { commit } => {
                if mine {
                    self.forget_sent(op.id);
                }
                if !self.keep.contains(&commit) {
                    self.keep.push(commit);
                }
                self.applied = op.sequence;
                self.take_heard_seal();
                self.seal_if_owed();
            }
        }
    }

    /// The order passed over `op`: every replica does alike. One of this replica's own is gone.
    fn passed(&mut self, op: &SessionOp, mine: bool) {
        self.applied = op.sequence;
        if mine && let Some(sent) = self.forget_sent(op.id) {
            self.lacking.dropped(op.id.counter);
            if let Some(id) = sent.reply {
                self.reply(id, Answer::Changed(false));
            }
            if self.reloading.contains(&op.id) {
                // The holder's reload of a merge never landed: it merges again.
                self.reloading.retain(|id| *id != op.id);
                self.jobs.push_back(Job::Reconcile);
            }
            self.in_flight = 0;
            self.rebuild_visible();
            self.cover_if_settled();
        }
        self.take_heard_seal();
    }

    /// Forgets what this replica submitted as `id`, which came back numbered.
    fn forget_sent(&mut self, id: OpId) -> Option<Sent<C>> {
        let at = self.pending.iter().position(|sent| sent.id == id)?;
        self.pending.remove(at)
    }

    fn apply_operation(&mut self, op: SessionOp, mine: bool, payload: Payload) {
        let id = op.id;
        let sequence = op.sequence;
        let readable = payload
            .part()
            .is_none_or(|part| part.length() <= LARGEST_PART);
        let operation = match op.base >= op.floor && readable {
            true => self.stamped(&payload, op.received),
            false => None,
        };
        let Some(operation) = operation else {
            if op.base >= op.floor && readable {
                // Bytes this build cannot decode: every replica of it passes over them too.
                self.incompatible = true;
            }
            return self.passed(&op, mine);
        };
        self.applied = sequence;
        self.log_operation(sequence, operation.clone(), mine);
        if mine && let Some(sent) = self.forget_sent(id) {
            let unchanged = sent.operation.as_ref().is_some_and(|submitted| {
                C::encode_operation(submitted) == C::encode_operation(&operation)
            });
            self.confirmed.apply(&operation);
            self.in_flight = 0;
            if !unchanged {
                self.rebuild_visible();
            }
            self.cover_if_settled();
            return self.take_heard_seal();
        }
        if mine {
            // Made by an earlier opening of this replica: the order holds it, and now the
            // content shows it.
            let bytes = match &payload {
                Payload::Inline(bytes) => bytes.clone(),
                Payload::Part(_) => Vec::new(),
            };
            self.lacking.journaled(&bytes, Some(sequence));
        }
        if self.pending.is_empty() && self.unsent.is_empty() && self.tentative.is_empty() {
            self.confirmed.apply(&operation);
            self.visible.apply(&operation);
            self.journal(Journaled::Applied(operation), None);
            return self.take_heard_seal();
        }
        // Another's edit, numbered before this replica's own: it goes beneath them.
        let unsettled = self.unsettled();
        if !self.covered {
            self.confirmed.apply(&operation);
            self.rebuild_visible();
            return self.take_heard_seal();
        }
        let mut local: Vec<C::Op> = self
            .pending
            .iter()
            .filter_map(|sent| sent.operation.clone())
            .collect();
        self.in_flight = local.len() as u64;
        for (pending, _, weight, ..) in &mut self.unsent {
            local.push(pending.clone());
            *weight = 1;
        }
        // The tentative layer stays on top.
        local.extend(self.tentative.iter().map(|held| held.operation.clone()));
        self.beneath = None;
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
        self.take_heard_seal();
    }

    /// A reload landed: the content becomes `content`, with what the order numbered after its
    /// base applied again on top unless it replaces the content outright.
    #[allow(clippy::too_many_arguments)]
    fn reloaded(
        &mut self,
        op: SessionOp,
        commit: CommitId,
        head: bool,
        replace: bool,
        restart: bool,
        state: Vec<u8>,
        _through: u64,
        content: C,
    ) {
        let mut content = content;
        let mine = op.id.origin == self.origin;
        match self.adopt_base_state(&state, op.sequence) {
            Some(state) if !state.is_empty() => {
                if content.adopt_session_state(&state).is_err() {
                    self.incompatible = true;
                }
            }
            Some(_) => {}
            None => self.incompatible = true,
        }
        if restart {
            self.restarts += 1;
        }
        let sent = match mine {
            true => self.forget_sent(op.id),
            false => None,
        };
        self.reloading.retain(|id| *id != op.id);
        let before = std::mem::replace(&mut self.confirmed, C::default());
        let mut reapplied = false;
        if !replace && !restart {
            for logged in self.log.iter().filter(|logged| logged.sequence > op.base) {
                content.apply(&logged.operation);
                reapplied = true;
            }
        }
        self.applied = op.sequence;
        self.confirmed = content;
        if replace {
            self.replaced_at = op.sequence;
            // What this replica had not had numbered, or had numbered after the replacement's
            // base, is dropped with what the replacement replaced: kept in history as a stash,
            // unless it is the replacement's own.
            let after_base = self
                .log
                .iter()
                .any(|logged| logged.mine && logged.sequence > op.base);
            let unsaved =
                !mine && (!self.pending.is_empty() || !self.unsent.is_empty() || after_base);
            // The tentative layer was never written, so is not kept.
            let lost = unsaved.then(|| self.unheld().clone());
            self.clear_history();
            if !mine {
                self.replaced_through = self.edit_number();
            }
            self.drop_unconfirmed();
            self.lacking.replaced();
            self.resnapshot = true;
            if let Some(lost) = lost {
                let written = self.written(&lost);
                self.stash(self.base, written, AfterStash::Keep);
            }
        }
        if restart {
            self.replaced_at = op.sequence;
            self.carry_pending(before);
            self.history.restart();
            self.restarted_at = Some(commit);
            self.log.clear();
        }
        match head {
            true => {
                self.base = Some(commit);
                if !reapplied {
                    self.sealed = op.sequence;
                    self.sealed_state = self.confirmed.session_state();
                    self.own_numbered.retain(|(at, _)| *at > op.sequence);
                    self.log.clear();
                    // The head holds the order through here: the journal starts from it.
                    if self.is_holder() {
                        self.rebase_journal();
                    }
                }
            }
            false => {
                if !self.keep.contains(&commit) {
                    self.keep.push(commit);
                }
            }
        }
        self.seal_if_owed();
        if let Some(id) = sent.and_then(|sent| sent.reply) {
            self.reply(id, Answer::Changed(true));
        }
        self.rebuild_visible();
        self.cover_if_settled();
        self.take_heard_seal();
        // The next edit goes while a seal is out too (`passes_a_seal`): left until the seal
        // lands, it would wait for the next edit to come.
        if self.waiting.is_none() || self.sealing_out() {
            self.flush();
        }
    }

    /// A stash the order keeps, or a reload, is sealed at once: what it changed reaches the
    /// head without waiting for a pause.
    fn seal_if_owed(&mut self) {
        let queued = self.jobs.iter().any(|job| matches!(job, Job::Seal { .. }));
        if self.is_holder() && self.owes() && !queued && !self.sealing {
            self.sealing = true;
            self.jobs.push_back(Job::Seal {
                sealing: Sealing::Due,
                reply: None,
            });
        }
    }

    fn drop_unconfirmed(&mut self) {
        let reloads: Vec<Sent<C>> = std::mem::take(&mut self.pending)
            .into_iter()
            .filter(|sent| sent.operation.is_none())
            .collect();
        self.pending = reloads.into();
        self.unsent.clear();
        self.kept_submissions.clear();
        self.in_flight = 0;
        self.covered = true;
    }

    fn is_clean(&self) -> bool {
        self.pending.is_empty()
            && self.unsent.is_empty()
            && self.doubtful.is_empty()
            && !self.owes()
    }

    /// Whether the holder owes a seal: the order went past its last seal, or a stash waits to
    /// be kept.
    fn owes(&self) -> bool {
        self.loaded && self.is_holder() && (self.applied > self.sealed || !self.keep.is_empty())
    }

    /// Whether a seal now would leave this holder owing less: no merge waiting for parts it
    /// could not read, no reload of its own on its way, and no retry waiting.
    fn seal_would_settle(&self) -> bool {
        self.owes()
            && !self.diverged
            && self.merge_failures == 0
            && self.reloading.is_empty()
            && self.seal_retry_at.is_none_or(|at| self.now >= at)
    }

    fn has_unsealed_work(&self) -> bool {
        self.applied > self.sealed || !self.keep.is_empty()
    }

    /// How many of this replica's own edits show on top of what the order applied: those in
    /// flight, those unsent, and the tentative layer's.
    fn unsettled(&self) -> u64 {
        self.in_flight
            + self.tentative.len() as u64
            + self
                .unsent
                .iter()
                .map(|(_, _, weight, ..)| weight)
                .sum::<u64>()
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

    fn edit(&mut self, mut operation: C::Op, origin: Option<EditorId>) -> bool {
        self.edited_since.get_or_insert(self.now);
        self.last_edit = Some(self.now);
        if self.diverged {
            self.error(DIVERGED);
            return false;
        }
        // Only the order stamps an arrival: taken from the editor and shown as it was, so it is
        // never submitted.
        if claims_arrival::<C>(&mut operation) {
            self.error(CLAIMED_ARRIVAL);
            self.reproject();
            return true;
        }
        // Made while a tentative layer is held: it goes beneath the layer, which stays on top.
        if !self.tentative.is_empty() {
            let mut unheld = self.unheld().clone();
            unheld.apply(&operation);
            let announcements = unheld.announcements(&operation);
            match self.doubtful_now {
                true => {
                    self.first_doubtful.get_or_insert(self.edit_number());
                    self.doubtful.push(operation);
                }
                false => self.queue(operation, self.edit_number(), announcements),
            }
            self.rebuild_visible();
            self.flush();
            return true;
        }
        self.visible.apply(&operation);
        if self.doubtful_now {
            self.journal(
                Journaled::Edited {
                    operation: operation.clone(),
                    unsettled: self.unsettled(),
                },
                origin,
            );
            self.first_doubtful.get_or_insert(self.edit_number());
            self.doubtful.push(operation);
            return true;
        }
        // Asked as the edit is made, so the host keeps it with the edit from here on.
        let announcements = self.visible.announcements(&operation);
        let unsettled = self.unsettled() + 1;
        self.journal(
            Journaled::Edited {
                operation: operation.clone(),
                unsettled,
            },
            origin,
        );
        self.queue(operation, self.edit_number(), announcements);
        self.flush();
        true
    }

    /// Queues `operation`, the local edit `through`, to submit, with what it announces.
    fn queue(
        &mut self,
        operation: C::Op,
        through: u64,
        announcements: Vec<be_block_content::Announcement>,
    ) {
        let payload = C::encode_operation(&operation);
        let size = payload.len();
        self.lacking.add(payload, announcements);
        let kept = self.kept_submission(through);
        let counter = kept.as_ref().map(|kept| kept.counter);
        let mut apart = false;
        let oversized = size > LARGEST_RELAYED_OPERATION;
        let leftover = match self.unsent.back_mut() {
            // Edits an earlier opening submitted go together, as that submission held them,
            // and nothing else goes with them.
            Some((last, bound, weight, last_through, last_kept))
                if last_kept.as_ref().map(|kept| kept.counter) == counter
                    && (counter.is_some()
                        || (!oversized && *bound + size <= LARGEST_RELAYED_OPERATION)) =>
            {
                let leftover = C::absorb_operation(last, operation);
                match leftover.is_none() {
                    true => {
                        // An upper bound: absorbing can shrink `last` (a later put replaces an
                        // earlier one), so a long burst starts a new operation sooner than its
                        // real size needs.
                        *bound += size;
                        *weight += 1;
                        *last_through = through;
                    }
                    // Not as that submission held them, which absorbing them the same way
                    // again should never find: a known fallback, they go anew.
                    false => {
                        debug_assert!(counter.is_none(), "kept edits absorbed apart");
                        *last_kept = None;
                        apart = true;
                    }
                }
                leftover
            }
            _ => Some(operation),
        };
        let kept = kept.filter(|_| {
            !apart
                && !self
                    .unsent
                    .iter()
                    .any(|(.., held)| held.as_ref().map(|held| held.counter) == counter)
        });
        self.unsent
            .extend(leftover.map(|operation| (operation, size, 1, through, kept)));
    }

    /// The submission of an earlier opening that holds the local edit `through`, letting go of
    /// those before it.
    fn kept_submission(&mut self, through: u64) -> Option<KeptSubmission> {
        while self
            .kept_submissions
            .front()
            .is_some_and(|kept| kept.through < through || kept.through <= self.own_through)
        {
            self.kept_submissions.pop_front();
        }
        self.kept_submissions.front().cloned()
    }

    /// The order numbered this replica's own submission `counter`: when an earlier opening
    /// made it, the edits given again that it holds are in the order now, and are not sent.
    fn heard_own(&mut self, counter: u64) {
        self.kept_submissions.retain(|kept| kept.counter != counter);
        let held = |(.., kept): &(C::Op, usize, u64, u64, Option<KeptSubmission>)| {
            kept.as_ref().is_some_and(|kept| kept.counter == counter)
        };
        if let Some(at) = self.unsent.iter().position(held)
            && let Some((_, _, weight, ..)) = self.unsent.remove(at)
        {
            self.lacking.submitted(counter, weight);
            self.rebuild_visible();
        }
    }

    /// Submits the next operation waiting, once nothing of this replica's is in flight: what is
    /// typed meanwhile is absorbed into one edit (decisions/a_replica_keeps_one_edit_in_flight.md).
    fn flush(&mut self) {
        // A put that failed goes again on its timer (`tick`), whatever asks meanwhile.
        if !self.loaded
            || self.released
            || self.put_retry_at.is_some()
            || self.diverged
            || self.putting()
            || self.pending.iter().any(|sent| sent.operation.is_some())
        {
            return;
        }
        let Some((operation, _, _, through, kept)) = self.unsent.front() else {
            return;
        };
        // An earlier opening's submission waits for the rest of the edits it holds, then goes
        // again as it was, its payload under its id, if they make it again exactly: otherwise
        // they go anew.
        if kept
            .as_ref()
            .is_some_and(|kept| self.edit_number() < kept.through)
        {
            return;
        }
        let again = kept
            .as_ref()
            .and_then(|kept| match Submitted::decode(&kept.payload) {
                Some(Submitted {
                    entry: Entry::Operation(Payload::Inline(bytes)),
                    ..
                }) if *through == kept.through && bytes == C::encode_operation(operation) => {
                    Some((kept.clone(), Payload::Inline(bytes)))
                }
                _ => None,
            });
        let payload = match &again {
            Some((_, payload)) => payload.clone(),
            None => match self.payload_of(C::encode_operation(operation)) {
                Some(payload) => payload,
                None => return,
            },
        };
        // The host keeps each operation's identity under the restart it reported last, and drops
        // those of an earlier one: told of a restart first, it keeps this one. What else
        // `report` sends is sent at the end of the run all the same, and each once.
        if self
            .reported
            .status
            .as_ref()
            .is_some_and(|status| status.restarts != self.restarts)
        {
            self.report();
        }
        let Some((operation, _, weight, through, _)) = self.unsent.pop_front() else {
            return;
        };
        self.in_flight = weight;
        let entry = Entry::Operation(payload.clone());
        let submission = match again {
            Some((kept, _)) => Submission {
                id: OpId {
                    origin: self.origin,
                    counter: kept.counter,
                },
                base: self.applied,
                barrier: false,
                payload: kept.payload,
            },
            None => self.submission(through, entry.clone(), false),
        };
        let announcements = self.lacking.submitted(submission.id.counter, weight);
        self.pending.push_back(Sent {
            id: submission.id,
            operation: Some(operation),
            payload,
            entry,
            reply: None,
        });
        self.out.push(Effect::Submit {
            replica: self.id,
            submission,
            announcements,
        });
    }

    /// A submission of `entry`, holding the local edits through `through`, based on what this
    /// replica applied.
    fn submission(&mut self, through: u64, entry: Entry, barrier: bool) -> Submission {
        self.counter += 1;
        Submission {
            id: OpId {
                origin: self.origin,
                counter: self.counter,
            },
            base: self.applied,
            barrier,
            payload: Submitted { through, entry }.encode(),
        }
    }

    /// The local edits a reload or a stash submitted now holds (`Submitted::through`): those
    /// a replacement on its way counts as in the order, otherwise those the order holds
    /// already. Never edits still unsent: a replica opened again after a crash would take them
    /// as ordered and not send them. Edits a replacement from elsewhere dropped count, as they
    /// are gone.
    fn entry_through(&self) -> u64 {
        match self.replacing_from {
            Some(from) => self.taken_before + from,
            None => self
                .own_through
                .max(self.taken_before)
                .max(self.replaced_through),
        }
    }

    /// Submits a reload or a stash to keep.
    fn submit_entry(&mut self, entry: Entry, barrier: bool, reply: Option<RequestId>) -> OpId {
        let through = self.entry_through();
        let submission = self.submission(through, entry.clone(), barrier);
        let id = submission.id;
        self.pending.push_back(Sent {
            id,
            operation: None,
            payload: Payload::Inline(Vec::new()),
            entry,
            reply,
        });
        self.out.push(Effect::Submit {
            replica: self.id,
            submission,
            announcements: Vec::new(),
        });
        id
    }

    fn submit_reload(
        &mut self,
        commit: CommitId,
        head: bool,
        replace: bool,
        state: Vec<u8>,
        reply: Option<RequestId>,
    ) {
        let applied = self.applied;
        self.submit_reload_based(commit, head, replace, state, reply, applied);
    }

    /// Submits a reload of `commit`, which holds the order through `through`: replicas apply
    /// again on top of it what was numbered after that.
    fn submit_reload_based(
        &mut self,
        commit: CommitId,
        head: bool,
        replace: bool,
        state: Vec<u8>,
        reply: Option<RequestId>,
        through: u64,
    ) {
        let restart = head && replace;
        let entry = Entry::Reload {
            commit,
            head,
            replace: replace && !restart,
            restart,
            state: self.base_state(state, self.applied),
        };
        let barrier = replace;
        let local = self.entry_through();
        self.counter += 1;
        let submission = Submission {
            id: OpId {
                origin: self.origin,
                counter: self.counter,
            },
            base: through,
            barrier,
            payload: Submitted {
                through: local,
                entry: entry.clone(),
            }
            .encode(),
        };
        let id = submission.id;
        if head {
            self.reloading.push(id);
        }
        self.pending.push_back(Sent {
            id,
            operation: None,
            payload: Payload::Inline(Vec::new()),
            entry,
            reply,
        });
        self.out.push(Effect::Submit {
            replica: self.id,
            submission,
            announcements: Vec::new(),
        });
    }

    /// How the host answered the submission `counter`.
    fn answer_submitted(&mut self, counter: u64, outcome: Outcome) {
        // A change waiting on what it submitted (a replacement) is answered once the server holds
        // it: a host that waits for the answer may be what would hand this replica the order.
        let reply = self
            .pending
            .iter_mut()
            .find(|sent| sent.id.counter == counter)
            .and_then(|sent| sent.reply.take());
        if let Some(id) = reply {
            let made = !matches!(outcome, Outcome::Failed(_) | Outcome::Refused(_));
            self.reply(id, Answer::Changed(made));
        }
        match outcome {
            Outcome::Held => self.lacking.held(counter),
            Outcome::Kept => {}
            Outcome::Failed(error) => {
                // It never reached the order: the host opens the replica again once it is back
                // in the session, giving it again what this one had not had numbered. (One
                // refused for a key not on the device yet is not failed: the host holds it and
                // submits it again once the key arrives.)
                self.error(format!("an edit did not reach the session: {error}"));
            }
            Outcome::Refused(error) => {
                // This account may not edit the block: sent again, it would be refused again.
                // It is given up as one the order passed over, shown as failed to its editors
                // until they let it go, and what is typed after goes on to be refused in turn.
                self.error(format!("an edit was not sent: {error}"));
                let at = self
                    .pending
                    .iter()
                    .position(|sent| sent.id.counter == counter);
                if at.and_then(|at| self.pending.remove(at)).is_some() {
                    self.lacking.dropped(counter);
                    self.in_flight = 0;
                    self.rebuild_visible();
                    self.cover_if_settled();
                    self.jobs.push_back(Job::Unblocked);
                }
            }
        }
    }

    /// Records `operation`, made in its editor's gesture `gesture` or outside any, for undo
    /// against the content it is applied to, beneath any tentative layer, and makes it.
    fn record_and_edit(
        &mut self,
        operation: C::Op,
        origin: Option<EditorId>,
        gesture: Option<u64>,
    ) -> bool {
        let at = self.now;
        self.unheld();
        let content = match &self.beneath {
            Some(beneath) if !self.tentative.is_empty() => beneath,
            _ => &self.visible,
        };
        self.history
            .record(content, &operation, origin, gesture, at);
        self.edit(operation, origin)
    }

    fn handle_editor(&mut self, editor: EditorId, message: FromEditor) {
        let Some(&editable) = self.editors.get(&editor) else {
            return;
        };
        match message {
            FromEditor::Operate {
                gesture,
                operation: bytes,
            } => {
                // Counted taken or refused, as the host counts what it relays (`attach`).
                *self.acknowledged.entry(editor).or_default() += 1;
                // The host's own edits are no one's to undo (`HOST_EDITOR`).
                let took = editable
                    && C::decode_operation(&bytes).is_ok_and(|operation| {
                        match (editor, self.held_now) {
                            // Made in its editor's open tentative gesture: held in the layer.
                            (_, Some((gesture, serial))) => self.hold(
                                gesture,
                                serial,
                                operation,
                                Some(editor),
                                editor != be_sdk_type_api::HOST_EDITOR,
                            ),
                            (be_sdk_type_api::HOST_EDITOR, None) => {
                                self.edit(operation, Some(editor))
                            }
                            _ => self.record_and_edit(operation, Some(editor), gesture),
                        }
                    });
                // A refused operation never lands as the editor's, so it is told what holds
                // now: otherwise it would take a later one landing as its own for this one.
                if !took && !self.holding.contains(&editor) {
                    self.snapshot_to(editor);
                }
            }
            FromEditor::FollowLacking => self.lacking.follow(editor),
            FromEditor::DismissLacking(id) => self.lacking.dismiss(id),
            FromEditor::Gesture { id, gesture: phase } if editable => {
                let gesture = GestureId {
                    editor: Some(editor),
                    id,
                };
                let operations = match phase {
                    // `follow` opened it, and ends a tentative gesture a message ends.
                    Gesture::BeginTentative => Vec::new(),
                    Gesture::Begin => {
                        // One already open under this name commits first, as a commit would.
                        self.history.commit(gesture);
                        self.history.begin(gesture, GestureKind::Group);
                        Vec::new()
                    }
                    Gesture::Commit => {
                        self.history.commit(gesture);
                        Vec::new()
                    }
                    Gesture::Cancel => {
                        self.unheld();
                        let content = match &self.beneath {
                            Some(beneath) if !self.tentative.is_empty() => beneath,
                            _ => &self.visible,
                        };
                        self.history.cancel(content, gesture)
                    }
                };
                for operation in operations {
                    self.edit(operation, None);
                }
            }
            FromEditor::Break if editable => self.history.split(Some(editor)),
            FromEditor::Replace(bytes) if editable => {
                if let Ok(content) = C::decode(&bytes) {
                    self.clear_history();
                    self.replacing_from.get_or_insert(self.taken);
                    self.jobs.push_front(Job::Replace {
                        content,
                        reply: None,
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
            FromEditor::Append { token, bytes } if editable && self.fold.is_some() => {
                let call = self.token();
                self.out.push(Effect::Append {
                    replica: self.id,
                    token: call,
                    bytes,
                });
                self.appends.insert(call, (editor, token));
            }
            FromEditor::Append { token, .. } => self.tell_editor(
                editor,
                &ToEditor::Appended {
                    token,
                    result: Err(match self.fold {
                        Some(_) => "the editor is attached read-only".to_owned(),
                        None => "this type keeps no log".to_owned(),
                    }),
                },
            ),
            FromEditor::ReadLog { token, from, limit } if self.fold.is_some() => {
                let call = self.token();
                self.out.push(Effect::ReadLog {
                    replica: self.id,
                    token: call,
                    from,
                    limit,
                });
                self.log_reads.insert(call, Some((editor, token)));
            }
            FromEditor::ReadLog { token, .. } => self.tell_editor(
                editor,
                &ToEditor::LogRead {
                    token,
                    result: Err("this type keeps no log".to_owned()),
                },
            ),
            FromEditor::PutPart { token, .. } => self.tell_editor(
                editor,
                &ToEditor::PartPut {
                    token,
                    result: Err("the editor is attached read-only".to_owned()),
                },
            ),
            FromEditor::Gesture { .. } | FromEditor::Break | FromEditor::Replace(_) => {}
        }
    }

    /// Keeps entries of the log heard or read, each id once at its first place, and has the
    /// description fold them; answers those it had not, or now tells better.
    fn take_log(
        &mut self,
        items: Vec<be_block_content::LogItem>,
    ) -> Vec<be_block_content::LogItem> {
        let mut fresh = Vec::new();
        for item in items {
            let seq = item.seq;
            match crate::editor::take_logged(&mut self.signed_log, &mut self.signed_ids, item) {
                crate::editor::Taken::Held => continue,
                crate::editor::Taken::New => self.log_grew = true,
                // Folded already as it was told then: the fold starts over.
                crate::editor::Taken::Changed => self.described = true,
            }
            fresh.extend(self.signed_log.get(&seq).cloned());
        }
        fresh
    }

    /// Entries appended to the log, which every editor hears.
    fn logged(&mut self, items: Vec<be_block_content::LogItem>) {
        let fresh = self.take_log(items);
        if fresh.is_empty() {
            return;
        }
        let editors: Vec<EditorId> = self.editors.keys().copied().collect();
        for editor in editors {
            self.tell_editor(editor, &ToEditor::Logged(fresh.clone()));
        }
    }

    fn answer_log_read(
        &mut self,
        token: Token,
        result: Result<(u64, Vec<be_block_content::LogItem>), String>,
    ) {
        let reader = self.log_reads.remove(&token).flatten();
        if let Ok((_, items)) = &result {
            self.take_log(items.clone());
        }
        match (reader, result) {
            (Some((editor, token)), result) => {
                self.tell_editor(editor, &ToEditor::LogRead { token, result });
            }
            // The replica's own read as it opens fails when the host is stopping, restarting
            // its module, or offline with nothing kept: it describes the content without the
            // log until it hears entries, and reads again as it next opens.
            (None, _) => {}
        }
    }

    fn answer_part_put(&mut self, put: Token, result: Result<be_block_content::PartRef, String>) {
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
            if let be_block_content::Part::Node { parts, .. } = part {
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
                be_block_content::Part::Bytes(bytes) => {
                    let from = usize::try_from(from).unwrap_or(usize::MAX);
                    let to = usize::try_from(to).unwrap_or(usize::MAX);
                    planned.push(Err(bytes[from..to].to_vec()));
                }
                be_block_content::Part::Reused(part) => {
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
                be_block_content::Part::Node { .. } => {}
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
                let Some(operations) = self
                    .visible
                    .child_operations(crate::be_child(change), self.client)
                else {
                    return self.reply(id, Answer::Changed(false));
                };
                for operation in operations {
                    self.record_and_edit(operation, None, None);
                }
                self.reply(id, Answer::Changed(true));
            }
            Change::FromFile(FileChange { name, bytes }) => {
                let current = self.unheld().encode();
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
            // The undo cancelled the tentative layer as it was taken (`follow`).
            Change::Undo if self.undo_spent => self.reply(id, Answer::Changed(true)),
            Change::Undo => {
                let stepped = self.history.undo(&self.visible);
                self.step_history(id, stepped);
            }
            Change::Redo => {
                let stepped = self.history.redo(&self.visible);
                self.step_history(id, stepped);
            }
        }
    }

    fn step_history(&mut self, id: RequestId, stepped: Stepped<C::Op>) {
        let changed = stepped.changed();
        for operation in stepped.operations {
            self.edit(operation, None);
        }
        self.reply(id, Answer::Changed(changed));
    }

    fn replace_with(&mut self, id: RequestId, content: Option<C>) {
        let Some(content) = content else {
            return self.reply(id, Answer::Changed(false));
        };
        self.clear_history();
        self.replacing_from.get_or_insert(self.taken);
        self.jobs.push_front(Job::Replace {
            content,
            reply: Some(id),
        });
    }

    /// Lets go of `replacing_from` once no replacement is queued or on its way.
    fn replace_settled(&mut self) {
        let queued = self
            .jobs
            .iter()
            .any(|job| matches!(job, Job::Replace { .. }));
        let waiting = matches!(
            &self.waiting,
            Some(Wait::Stash {
                then: AfterStash::Unsaved { .. } | AfterStash::Replacement { .. },
                ..
            })
        );
        let landing = self
            .pending
            .iter()
            .any(|sent| matches!(sent.entry, Entry::Reload { replace: true, .. }));
        if !queued && !waiting && !landing {
            self.replacing_from = None;
        }
    }

    fn attach(&mut self, editor: EditorId, editable: bool, acknowledged: u64) {
        let first = self.editors.is_empty();
        if self.editors.insert(editor, editable).is_some() {
            return;
        }
        self.acknowledged.insert(editor, acknowledged);
        // Its snapshot waits for `resent`, after what the host gives again of its edits.
        self.holding.insert(editor);
        if first {
            self.journal(Journaled::Replaced, None);
        }
        if !self.presence.is_empty() {
            let presence = self.peer_presence();
            self.tell_editor(editor, &ToEditor::Presence(presence));
        }
    }

    /// The host gave this replica again what `editor` sent before it was opened (`resent`):
    /// the editor's snapshot holds all of it now, as `applied` counts.
    fn resent(&mut self, editor: EditorId) {
        if !self.holding.remove(&editor) || !self.editors.contains_key(&editor) {
            return;
        }
        if let Some(reason) = &self.unreadable {
            let failed = ToEditor::Failed(format!("The content could not be read: {reason}"));
            return self.tell_editor(editor, &failed);
        }
        // Opened again in place of a replica that told the editor this snapshot (its block's
        // session joined once its creation was sent), it shows it already.
        let snapshot = ToEditor::Snapshot(self.snapshot_for(editor));
        let bytes = snapshot.encode();
        if self.snapshots_shown.get(&editor) != Some(&be_block_store::Hash::of(&bytes)) {
            self.send_editor(editor, &snapshot, bytes);
        }
    }

    fn snapshot_to(&mut self, editor: EditorId) {
        self.holding.remove(&editor);
        if let Some(reason) = &self.unreadable {
            let failed = ToEditor::Failed(format!("The content could not be read: {reason}"));
            return self.tell_editor(editor, &failed);
        }
        let snapshot = self.snapshot_for(editor);
        self.tell_editor(editor, &ToEditor::Snapshot(snapshot));
    }

    fn snapshot_for(&self, editor: EditorId) -> Snapshot {
        let applied = self.acknowledged.get(&editor).copied().unwrap_or_default();
        let settled = self.projected.settled();
        let bytes = settled.encode();
        let length = match C::LOADS {
            be_block_content::Loads::Whole => bytes.len() as u64,
            be_block_content::Loads::FirstPart => settled.parts().length(),
        };
        Snapshot {
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
        }
    }

    /// Tells the editors following `ToEditor::Lacking` what changed: edits the order numbered,
    /// or the server holds, since it last told them.

    /// Tells the editors following `ToEditor::Lacking` what changed.
    fn tell_lacking(&mut self) {
        for (editor, message) in self.lacking.messages() {
            self.tell_editor(editor, &message);
        }
    }

    fn tell_editor(&mut self, editor: EditorId, message: &ToEditor) {
        let bytes = message.encode();
        self.send_editor(editor, message, bytes);
    }

    /// Sends `message`, encoded as `bytes`, to `editor`.
    fn send_editor(&mut self, editor: EditorId, message: &ToEditor, bytes: Vec<u8>) {
        match message {
            ToEditor::Snapshot(_) => {
                self.snapshots_shown
                    .insert(editor, be_block_store::Hash::of(&bytes));
            }
            ToEditor::Operations(_) => {
                self.snapshots_shown.remove(&editor);
            }
            _ => {}
        }
        self.out.push(Effect::Send {
            replica: self.id,
            to: Recipients::One(Endpoint::Editor(editor)),
            bytes,
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
        // One still held (`attach`) is sent everything at once in its snapshot.
        let shown: Vec<EditorId> = editors
            .iter()
            .copied()
            .filter(|editor| !self.holding.contains(editor))
            .collect();
        if std::mem::take(&mut self.resnapshot) {
            self.landed.clear();
            for editor in &shown {
                self.snapshot_to(*editor);
            }
        }
        let landed = std::mem::take(&mut self.landed);
        if !landed.is_empty() {
            let encoded: Vec<(Landed<Vec<u8>>, Option<EditorId>)> = landed
                .iter()
                .map(|(landed, origin)| (landed.map(C::encode_operation), *origin))
                .collect();
            for editor in &shown {
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

    /// What keeps edits out of the journal (`Status::unjournaled`), with the timers it waits on.
    fn unjournaled(&self) -> Unjournaled {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Unjournaled {
            loaded: self.loaded,
            pending: count(self.pending.len()),
            unsent: count(self.unsent.len()),
            doubtful: count(self.doubtful.len()),
            putting: self.putting(),
            put_retry_at: self.put_retry_at,
            merge_retry_at: self.merge_retry_at,
            seal_retry_at: self.seal_retry_at,
            merge_failures: self.merge_failures,
            resubmits_through: self
                .unsent
                .front()
                .and_then(|(.., kept)| kept.as_ref())
                .map(|kept| kept.through)
                .filter(|through| self.edit_number() < *through),
        }
    }

    fn report(&mut self) {
        let clean = self.loaded && self.is_clean();
        let unconfirmed =
            !self.pending.is_empty() || !self.unsent.is_empty() || !self.doubtful.is_empty();
        // Nothing from the first doubtful edit on is held anywhere but by the host.
        let doubtful_cap = self
            .first_doubtful
            .map(|first| first.saturating_sub(1).saturating_sub(self.taken_before));
        let ordered = match self.replacing_from {
            Some(from) => from.saturating_sub(1),
            None => self.taken,
        };
        // What the order holds of the local edits: all of them once nothing waits, otherwise as
        // far as its own entries came back.
        let journaled_taken = match unconfirmed {
            false => ordered,
            true => self
                .own_through
                .saturating_sub(self.taken_before)
                .min(ordered),
        };
        let journaled_taken = doubtful_cap.map_or(journaled_taken, |cap| journaled_taken.min(cap));
        let sealed_taken = match clean {
            true => ordered,
            false => self
                .sealed_taken
                .saturating_sub(self.taken_before)
                .min(journaled_taken),
        };
        let undo = (self.loaded && !self.released).then(|| UndoState {
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
            restarted_at: self.restarted_at.filter(|_| self.history.stopped()),
            lost_edits: self.lost_edits,
        });
        let status = Status {
            head: self.base,
            holder: self.is_holder(),
            restarts: self.restarts,
            clean: self.is_clean(),
            journaled: !unconfirmed,
            unjournaled: unconfirmed.then(|| self.unjournaled()),
            owes_seal: self.loaded && self.seal_would_settle(),
            taken: self.taken,
            sealed_taken,
            journaled_taken: journaled_taken.max(sealed_taken),
            epoch: self.epoch,
            incompatible: self.incompatible,
            diverged: self.diverged,
            unreadable: self.unreadable.clone(),
            undo,
        };
        // Since when there is work to seal, whether or not a seal now would settle it: a seal
        // that waits on a merge or a retry is still timed from then.
        self.owes_since = match self.owes() {
            true => self
                .owes_since
                .or(Some(self.edited_since.unwrap_or(self.now).min(self.now))),
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
        let changed = std::mem::take(&mut self.described);
        let grew = std::mem::take(&mut self.log_grew);
        if changed || grew {
            let organization = self.organization;
            let description = match self.fold {
                // What the content shows with its log folded onto it, in the log's order: the
                // run of entries from its start this replica holds, up to the first it lacks,
                // and each signed as nobody's (`Bad`) left out. Content changed folds again from
                // the start; entries that follow on fold onto what was folded.
                Some(fold) => {
                    if changed || self.folded.is_none() {
                        self.folded = Some((self.unheld().clone(), 0));
                    }
                    let (folded, through) = self.folded.as_mut().expect("folded is kept above");
                    let from = *through;
                    while let Some(item) = self.signed_log.get(&*through) {
                        if item.signer != be_block_content::LogSigner::Bad {
                            fold(folded, item);
                        }
                        *through += 1;
                    }
                    if !changed && *through == from {
                        return;
                    }
                    crate::describe(folded, organization)
                }
                None => crate::describe(self.unheld(), organization),
            };
            self.out.push(Effect::Described {
                replica: self.id,
                description,
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
        self.out.push(Effect::Send {
            replica: self.id,
            to: match to {
                Some(client) => Recipients::One(Endpoint::Peer(client)),
                None => Recipients::Peers,
            },
            bytes: wire::encode(message),
        });
    }

    /// Shows what is confirmed with this replica's own edits on top: what it submitted that has
    /// not come back, then what it has not submitted, then the tentative layer.
    fn rebuild_visible(&mut self) {
        let mut visible = self.confirmed.clone();
        for sent in &self.pending {
            if let Some(operation) = &sent.operation {
                visible.apply(operation);
            }
        }
        for (operation, ..) in &self.unsent {
            visible.apply(operation);
        }
        for operation in &self.doubtful {
            visible.apply(operation);
        }
        self.beneath = None;
        if !self.tentative.is_empty() {
            self.beneath = Some(visible.clone());
            for held in &self.tentative {
                visible.apply(&held.operation);
            }
        }
        self.replace_visible(visible);
    }

    fn adopt(&mut self, state: SessionState) {
        let known: BTreeSet<ClientId> = self.state.clients().into_iter().collect();
        let joined: Vec<ClientId> = state
            .clients()
            .into_iter()
            .filter(|client| *client != self.client && !known.contains(client))
            .collect();
        let before = self.presence.len();
        self.presence.retain(|(client, _), _| state.has(*client));
        self.presence_changed |= self.presence.len() != before;
        let was_holder = self.is_holder();
        self.state = state;
        self.check_build();
        for client in joined {
            self.show_presence_to(client);
        }
        if was_holder && !self.is_holder() {
            // A merge that was failing is the new holder's to do.
            self.merge_failures = 0;
            self.merge_retry_at = None;
            self.reloading.clear();
        }
    }

    /// Claims the lease, as a terminal host does: the server writes the claim into the order,
    /// and the lease follows from it.
    fn claim(&mut self) {
        if !self.loaded || self.is_holder() {
            return;
        }
        self.out.push(Effect::Claim { replica: self.id });
    }

    /// A head landed: the holder reconciles with it. Any other replica leaves that to the
    /// holder, which hears of every head too.
    fn moved(&mut self, head: CommitId) {
        if self.base != Some(head) && self.is_holder() {
            self.jobs.push_front(Job::Reconcile);
        }
    }

    fn reconcile(&mut self) {
        if !self.is_holder() || !self.loaded || self.diverged {
            return;
        }
        let token = self.token();
        self.out.push(Effect::Resume {
            replica: self.id,
            token,
            local: self.base,
        });
        self.waiting = Some(Wait::Resume { token });
    }

    fn seal(&mut self, sealing: Sealing, reply: Option<RequestId>) {
        let skip = match sealing {
            Sealing::Due => !self.is_holder() || !self.owes(),
            Sealing::Rekey => !self.is_holder() || self.diverged,
            Sealing::Once => !self.is_holder(),
        };
        // A reload of its own on its way: the seal waits for it, as it would publish on the head
        // the reload replaces.
        if skip || !self.reloading.is_empty() {
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
        let kept = self.keep.first().copied();
        let content = self.written(&self.confirmed);
        let merging = kept.map(|ours| Merging {
            base: self.base,
            ours,
        });
        let state = self.confirmed.session_state();
        let through = self.applied;
        self.publish(
            self.base,
            merging,
            content,
            AfterPublish::Seal {
                retry: sealing != Sealing::Once,
                reply,
                kept,
                through,
                state,
            },
        );
    }

    /// Replaces the content outright: what this replica has not seen sealed is stashed, the
    /// replacement is stashed on top of it, and the order takes it as a barrier, so it wins over
    /// whatever was based before it. Every replica then shows it.
    fn replace(&mut self, content: C, reply: Option<RequestId>) {
        if self.diverged {
            self.error(DIVERGED);
            if let Some(id) = reply {
                self.reply(id, Answer::Changed(false));
            }
            return self.replace_settled();
        }
        // What it replaces stays in history: its unsaved work, and the head, which only a
        // holder's seal would otherwise keep (`Replicas::bookmark`).
        let kept = self.unheld().clone();
        let kept = self.written(&kept);
        self.stash(self.base, kept, AfterStash::Unsaved { content, reply });
    }

    fn restart_due(&self) -> bool {
        if self.history.gesture_open() || self.holds.holding() {
            return false;
        }
        let state = self.confirmed.session_state().len();
        let limit = restart_limit();
        state > limit
            && state > self.confirmed.encode().len()
            && (self.alone() || state > limit.saturating_mul(RESTART_CEILING))
    }

    /// Moves this replica's own edits not yet numbered from `before` onto the content it just
    /// reloaded at a restart: they go again, after it.
    fn carry_pending(&mut self, before: C) {
        let mut before = before;
        let mut after = self.confirmed.clone();
        let mut carried = VecDeque::new();
        let pending: Vec<C::Op> = std::mem::take(&mut self.pending)
            .into_iter()
            .filter_map(|sent| sent.operation)
            .collect();
        let unsent: Vec<(C::Op, u64)> = std::mem::take(&mut self.unsent)
            .into_iter()
            .map(|(operation, _, _, through, _)| (operation, through))
            .collect();
        let through = self.edit_number();
        for (operation, through) in pending
            .into_iter()
            .map(|operation| (operation, through))
            .chain(unsent)
        {
            let moved = C::carry_over(&operation, &before, &after, self.client);
            before.apply(&operation);
            if let Some(moved) = moved {
                after.apply(&moved);
                let size = C::encode_operation(&moved).len();
                carried.push_back((moved, size, 1, through, None));
            }
        }
        self.unsent = carried;
        self.in_flight = 0;
        // The tentative layer goes on after them, on the same chain; a gesture with an edit
        // that does not carry ends, its edits dropped.
        let mut lost = Vec::new();
        for held in std::mem::take(&mut self.tentative) {
            let moved = C::carry_over(&held.operation, &before, &after, self.client);
            before.apply(&held.operation);
            match moved {
                Some(moved) if !lost.contains(&held.gesture) => {
                    after.apply(&moved);
                    self.tentative.push(Held {
                        operation: moved,
                        ..held
                    });
                }
                _ => lost.push(held.gesture),
            }
        }
        for gesture in lost {
            self.void(gesture);
        }
        // What earlier openings submitted was made before the restart, and is passed over.
        self.kept_submissions.clear();
        // They go again as new submissions: the ones on their way are passed over.
        self.lacking.unsubmitted();
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

/// The bytes an entry adds to what the order holds since the last seal: its own, and those of
/// the part it names, which no commit holds either.
fn ordered_bytes(op: &SessionOp) -> usize {
    let parted = match Submitted::decode(&op.payload) {
        Some(Submitted {
            entry: Entry::Operation(Payload::Part(part)),
            ..
        }) => usize::try_from(part.length()).unwrap_or(usize::MAX),
        _ => 0,
    };
    op.payload.len().saturating_add(parted)
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
    let state = crate::wire::BaseState::decode(&journal.base.state)
        .ok_or("the journal's base state does not decode")?
        .content;
    if !state.is_empty() {
        content
            .adopt_session_state(&state)
            .map_err(|error| error.to_string())?;
    }
    for entry in &journal.entries {
        if entry.sequence <= journal.base.sequence || entry.base < entry.floor {
            continue;
        }
        let Some(Submitted { entry: ordered, .. }) = Submitted::decode(&entry.payload) else {
            return Err("a journal entry this build cannot read".into());
        };
        match ordered {
            Entry::Operation(Payload::Inline(bytes)) => {
                let mut operation =
                    C::decode_operation(&bytes).map_err(|error| error.to_string())?;
                if let Some(slot) = C::arrival(&mut operation) {
                    *slot = entry.received;
                }
                content.apply(&operation);
            }
            // Only a replica reads an operation's part, or a reload's commit.
            Entry::Operation(Payload::Part(_)) => {
                return Err("a journal entry names an operation by its part".into());
            }
            Entry::Reload { .. } => return Err("a journal entry reloads a commit".into()),
            Entry::Keep { .. } => {}
        }
    }
    Ok(content.encode())
}
