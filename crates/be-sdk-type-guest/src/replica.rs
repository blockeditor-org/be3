use std::collections::{BTreeMap, BTreeSet, VecDeque};

use be_block::{Landed, LiveEdit, Merge, Settling};
use be_block_commit::{CommitId, MergeResult};
use be_block_protocol::{ClientId, SessionState};
use be_block_session::{
    Follower, Inherited, KeptEdits, OpId, Resume, Sequencer, SessionMessage, SessionOp,
};
use be_sdk_type_api::{
    Change, EditorId, Endpoint, FileChange, Journal, JournalBase, JournalEntry, Merging,
    Recipients, ReplicaId, SessionTag, Status, UndoState, Written,
};
use uuid::Uuid;

use crate::{
    FromFileFn, Recorder,
    editor::{EditorOperation, FromEditor, Gesture, PeerMessage, PeerPresence, Snapshot, ToEditor},
    effect::{Answer, Effect, HeadEvent, RequestId, Token},
    wire::Wire,
};

/// An owner commits once its session has gone this long with no edits: a natural pause,
/// which is the state retention prefers to keep.
pub const QUIET_INTERVAL: u64 = 30_000;

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
const LARGEST_RELAYED_OPERATION: usize = 1024 * 1024;
const REPLACE_ATTEMPTS: u8 = 4;
const OP_HEADS: usize = 64;
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
    pub(crate) lost_edits: bool,
    pub(crate) from_file: Option<FromFileFn>,
    pub(crate) now: u64,
}

pub(crate) trait AnyReplica {
    fn start(&mut self, out: &mut Vec<Effect>);

    fn input(&mut self, now: u64, input: Input, out: &mut Vec<Effect>);

    fn tick(&mut self, now: u64, out: &mut Vec<Effect>);

    fn wake_at(&self) -> Option<u64>;

    fn released(&self) -> bool;
}

pub(crate) enum Input {
    Message {
        from: Endpoint,
        bytes: Vec<u8>,
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
    Oversized,
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
    edits: Option<KeptEdits>,
}

#[derive(Clone, Copy)]
struct Keeping {
    commit: CommitId,
    base: Option<CommitId>,
    edits: Option<KeptEdits>,
    from: ClientId,
    attempts: u8,
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
            edits: self.edits,
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
    Absorb {
        head: CommitId,
    },
    Replacement,
    Handed {
        keeping: Keeping,
        replay: bool,
    },
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
        merged: Option<C>,
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
    Ancestor {
        content: C,
        applied: u64,
    },
    Hand {
        content: C,
        applied: u64,
        ancestor: Option<CommitId>,
    },
    Handed {
        keeping: Keeping,
        value: C,
        count: usize,
    },
}

enum Wait<C> {
    Reads { slots: Vec<Slot<C>>, then: Then },
    Resume { token: Token },
    Publish { token: Token, then: AfterPublish<C> },
    Stash { token: Token, then: AfterStash<C> },
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
    state: SessionState,
    role: Role,
    confirmed: C,
    visible: C,
    unsent: VecDeque<(C::Op, usize, u64)>,
    held: Vec<C::Op>,
    in_flight: u64,
    covered: bool,
    base: Option<CommitId>,
    sealed: u64,
    sealed_state: Vec<u8>,
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
    rebased: bool,
    unappended: Vec<SessionOp>,
    append_at: Option<u64>,
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
    op_heads: Vec<Option<CommitId>>,
    saved: VecDeque<CommitId>,
    loaded: bool,
    released: bool,
    reported: Reported,
    ranges: Vec<RangeRead>,
    /// Parts editors asked to put, by the token of the host call: the editor and its token.
    puts: BTreeMap<Token, (EditorId, u64)>,
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
            .chain(self.beat_at())
            .min()
    }

    fn released(&self) -> bool {
        self.released
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
            state: opened.session,
            role: Role::Follower(Follower::new(opened.client)),
            projected: Settling::new(empty.clone(), Vec::new()),
            sealed_state: Vec::new(),
            visible: empty.clone(),
            confirmed: empty,
            unsent: VecDeque::new(),
            held: Vec::new(),
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
            rebased: false,
            unappended: Vec::new(),
            append_at: None,
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
            op_heads: Vec::new(),
            saved: VecDeque::new(),
            loaded: false,
            released: false,
            reported: Reported::default(),
            ranges: Vec::new(),
            puts: BTreeMap::new(),
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
            let Some(job) = self.jobs.pop_front() else {
                break;
            };
            self.job(job);
        }
        match self.loaded && self.is_owner() && !self.released {
            true => {
                self.beaten.get_or_insert(self.now);
            }
            false => self.beaten = None,
        }
        self.report();
        out.append(&mut self.out);
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

    fn sequence_held(&mut self) {
        for operation in std::mem::take(&mut self.held) {
            let Role::Owner(sequencer) = &mut self.role else {
                let size = C::encode_operation(&operation).len();
                self.unsent.push_back((operation, size, 1));
                continue;
            };
            let id = OpId {
                client: self.client,
                counter: sequencer.sequence() + 1,
            };
            if let Some(op) = sequencer.accept(id, C::encode_operation(&operation)) {
                self.confirmed.apply(&operation);
                self.sequenced(op);
            }
        }
        self.flush();
    }

    /// When the owner commits what it owes: after a quiet spell, at most `COMMIT_INTERVAL`
    /// after it started owing, or at once when the journal grew past `JOURNAL_BYTES`.
    fn commit_at(&self) -> Option<u64> {
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
        match self.alone() {
            true => {
                let folded = fold::<C>(ops);
                self.append(folded, false);
            }
            false => self.append(ops, true),
        }
    }

    fn append(&mut self, ops: Vec<SessionOp>, relay: bool) {
        if !self.is_owner() || ops.is_empty() {
            return;
        }
        if !self.rebased {
            self.rebase_journal();
        }
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
        self.out.push(Effect::Append {
            replica: self.id,
            generation: self.state.generation,
            entries,
            relay,
        });
    }

    /// Starts the journal from this owner's base: its last commit, the sequence number that
    /// commit holds, and the session state as of it.
    fn rebase_journal(&mut self) {
        self.rebased = true;
        self.out.push(Effect::Rebase {
            replica: self.id,
            generation: self.state.generation,
            base: JournalBase {
                commit: self.base,
                sequence: self.sealed,
                state: self.sealed_state.clone(),
            },
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
            Job::Seal { sealing, reply } => self.seal(sealing, reply),
            Job::Reconcile => self.reconcile(),
            Job::Replace {
                content,
                reply,
                outright,
            } => self.replace(content, reply, outright),
            Job::Keep(keeping) => self.keep(keeping),
            Job::Oversized => self.oversized(),
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
                from: Endpoint::Peer(from),
                bytes,
            } => {
                match Wire::decode(&bytes) {
                    Some(wire) => self.receive(from, wire),
                    None => self.unreadable_message(),
                }
                if self.waiting.is_none() {
                    self.flush();
                }
            }
            Input::Message {
                from: Endpoint::Editor(editor),
                bytes,
            } => {
                if let Some(message) = FromEditor::decode(&bytes) {
                    self.handle_editor(editor, message);
                }
                self.reported.status = None;
            }
            Input::Message {
                from: Endpoint::Host,
                ..
            } => {}
            Input::Attach { editor, editable } => self.attach(editor, editable),
            Input::Detach { editor } => {
                self.editors.remove(&editor);
            }
            Input::Head(HeadEvent::Moved {
                head,
                from_this_device,
            }) => self.moved(head, from_this_device),
            Input::Head(_)
            | Input::Read { .. }
            | Input::PartRead { .. }
            | Input::PartPut { .. }
            | Input::Resumed { .. } => {}
            Input::Lease {
                session,
                resynchronize,
            } => {
                self.adopt(session);
                if resynchronize && let Role::Follower(_) = &self.role {
                    self.catching_up = true;
                    self.greet();
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
            Then::Load => self.loaded(next()),
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
            Then::Absorb { head } => {
                self.replacement(head, next());
                self.flush();
            }
            Then::Replacement => {
                self.history.clear();
                self.replace(next(), None, true);
            }
            Then::Handed { keeping, replay } => {
                let (value, conflicts) = match replay {
                    true => (self.replayed(next(), &keeping), None),
                    false => {
                        let ancestor = next();
                        let theirs = next();
                        match C::merge3(&ancestor, &self.confirmed, &theirs) {
                            MergeResult::Clean(value) => (value, None),
                            MergeResult::Conflicted { value, conflicts } => {
                                (value, Some(conflicts))
                            }
                        }
                    }
                };
                match conflicts {
                    None => self.publish_kept(keeping, Some(value)),
                    Some(count) => {
                        let content = self.written(&self.confirmed);
                        self.stash(
                            self.base,
                            content,
                            AfterStash::Handed {
                                keeping,
                                value,
                                count,
                            },
                        );
                    }
                }
            }
            Then::FastForward { to } => {
                self.reached = Some(to);
                self.confirmed = next();
                self.replace_visible(self.confirmed.clone());
                self.reload = true;
                self.settle_at(to);
            }
            Then::FastForwardMerge { to } => {
                let ancestor = next();
                let theirs = next();
                match C::merge3(&ancestor, &self.confirmed, &theirs) {
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
                }
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
                let (value, conflicts) = match C::merge3(&ancestor, &ours_content, &theirs_content)
                {
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
        self.out.push(Effect::Publish {
            replica: self.id,
            token,
            expected,
            merging,
            content,
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
        match (self.waiting.take(), event) {
            (Some(Wait::Publish { then, .. }), HeadEvent::Published { head, .. }) => {
                self.published(then, Some(head));
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
                        self.replace_visible(self.confirmed.clone());
                        self.reload = true;
                        self.broadcast(&SessionMessage::Replaced { head });
                        self.settle_at(head);
                    }
                    (Role::Owner(_), false) => {
                        self.confirmed = content;
                        self.replace_visible(self.confirmed.clone());
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
                    (Role::Follower(_), false) => {
                        self.unsent.clear();
                        self.replace_visible(content);
                        let owner = self.state.owner;
                        self.send(owner, &SessionMessage::Moved { head });
                    }
                }
                if let Some(id) = reply {
                    self.reply(id, Answer::Changed(true));
                }
            }
            AfterPublish::Merged { value } => self.take_merged(value, head),
            AfterPublish::Kept { keeping, merged } => {
                let Some(head) = head else {
                    return;
                };
                if let Some(value) = merged {
                    self.confirmed = value;
                    self.replace_visible(self.confirmed.clone());
                    self.reload = true;
                }
                if self.saved.len() >= OP_HEADS {
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
                self.drop_unconfirmed();
                self.rebuild_visible();
                self.hand(Handed {
                    commit,
                    base: self.base,
                    edits: None,
                });
            }
            AfterStash::Ancestor { content, applied } => {
                let written = self.written(&content);
                self.stash(
                    Some(commit),
                    written,
                    AfterStash::Hand {
                        content,
                        applied,
                        ancestor: Some(commit),
                    },
                );
            }
            AfterStash::Hand {
                content,
                applied,
                ancestor,
            } => {
                self.drop_unconfirmed();
                self.confirmed = content;
                self.rebuild_visible();
                let sealed = self.base;
                self.hand(Handed {
                    commit,
                    base: ancestor,
                    edits: Some(KeptEdits { applied, sealed }),
                });
            }
            AfterStash::Handed {
                keeping,
                value,
                count,
            } => {
                self.conflict(commit, keeping.commit, count);
                self.jobs.push_front(Job::Keep(Keeping {
                    commit,
                    base: self.base,
                    edits: None,
                    from: self.client,
                    attempts: 0,
                }));
                self.publish_kept(keeping, Some(value));
            }
        }
    }

    fn acknowledge(&mut self, keeping: &Keeping) {
        match keeping.from == self.client {
            true => self.handed.retain(|handed| handed.commit != keeping.commit),
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

    fn handing_edits(&self) -> bool {
        self.handed.iter().any(|handed| handed.edits.is_some())
    }

    fn publish_kept(&mut self, keeping: Keeping, merged: Option<C>) {
        let content = self.written(merged.as_ref().unwrap_or(&self.confirmed));
        self.publish(
            self.base,
            Some(Merging {
                base: keeping.base,
                ours: keeping.commit,
            }),
            content,
            AfterPublish::Kept { keeping, merged },
        );
    }

    fn replayed(&self, mut value: C, keeping: &Keeping) -> C {
        let (Role::Owner(sequencer), Some(edits)) = (&self.role, keeping.edits) else {
            return value;
        };
        for op in sequencer.since(edits.applied) {
            if op.id.client == keeping.from {
                continue;
            }
            if let Ok(operation) = C::decode_operation(&op.payload) {
                value.apply(&operation);
            }
        }
        value
    }

    fn rejected(&mut self, then: AfterPublish<C>, head: Option<CommitId>) {
        match then {
            AfterPublish::Seal { retry: true, reply } => {
                self.jobs.push_front(Job::Seal {
                    sealing: Sealing::Once,
                    reply,
                });
                self.jobs.push_front(Job::Reconcile);
            }
            AfterPublish::Seal {
                retry: false,
                reply,
            } => {
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

    fn loaded(&mut self, content: C) {
        self.confirmed = content;
        self.sealed_state = self.confirmed.session_state();
        let journal = self
            .opened_journal
            .take()
            .filter(|journal| journal.base.commit == self.base);
        let replayed = match journal {
            Some(journal) => self.replay_journal(journal),
            None => Vec::new(),
        };
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
        if self.is_owner() {
            self.op_heads = vec![self.base];
        }
        if let Role::Follower(_) = &self.role {
            self.catching_up = true;
            self.greet();
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
        let mut replayed = Vec::new();
        for entry in journal.entries {
            let Some(Wire::Session(SessionMessage::Accepted { op })) = Wire::decode(&entry.payload)
            else {
                continue;
            };
            let Ok(operation) = C::decode_operation(&op.payload) else {
                continue;
            };
            self.confirmed.apply(&operation);
            self.appended_bytes += entry.payload.len();
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
        if oversized && self.is_owner() {
            self.hand_over_oversized();
            return true;
        }
        let holding = self.sealing_out();
        match &mut self.role {
            Role::Owner(_) if holding => self.held.push(operation),
            Role::Owner(sequencer) => {
                let id = OpId {
                    client: self.client,
                    counter: sequencer.sequence() + 1,
                };
                let Some(op) = sequencer.accept(id, payload) else {
                    return true;
                };
                self.confirmed.apply(&operation);
                self.sequenced(op);
            }
            Role::Follower(follower)
                if oversized
                    || follower.pending() > 0
                    || !self.unsent.is_empty()
                    || self.awaiting.is_some() =>
            {
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
                let message = follower.submit(payload);
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
                self.puts.insert(put, (editor, token));
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
        if let Some((editor, token)) = self.puts.remove(&put) {
            self.tell_editor(editor, &ToEditor::PartPut { token, result });
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
            applied: self.acknowledged.get(&editor).copied().unwrap_or_default(),
        };
        self.tell_editor(editor, &ToEditor::Snapshot(snapshot));
    }

    fn tell_editor(&mut self, editor: EditorId, message: &ToEditor) {
        self.out.push(Effect::Send {
            replica: self.id,
            to: Recipients::One(Endpoint::Editor(editor)),
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
                    | SessionMessage::Moved { .. }
                    | SessionMessage::Kept { .. }
            )
        {
            return;
        }
        match message {
            SessionMessage::Submit { id, base, payload } => {
                let Role::Owner(sequencer) = &mut self.role else {
                    return;
                };
                if base < self.replaced_at {
                    return;
                }
                let Ok(operation) = C::decode_operation(&payload) else {
                    return self.unreadable_message();
                };
                let onto: Vec<_> = sequencer
                    .since(base)
                    .iter()
                    .filter_map(|op| C::decode_operation(&op.payload).ok())
                    .collect();
                let Some(operation) = C::rebase(operation, &onto) else {
                    return;
                };
                let Some(op) = sequencer.accept(id, C::encode_operation(&operation)) else {
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
            SessionMessage::Moved { head } => {
                if !self.is_owner() || self.base == Some(head) {
                    return;
                }
                self.read(vec![Some(head)], Then::Absorb { head });
            }
            SessionMessage::Kept {
                commit,
                base,
                edits,
            } => {
                if self.is_owner() {
                    self.jobs.push_front(Job::Keep(Keeping {
                        commit,
                        base,
                        edits,
                        from,
                        attempts: 0,
                    }));
                }
            }
            SessionMessage::Saved { commit } => {
                self.handed.retain(|handed| handed.commit != commit);
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
                if reload.is_some() && self.handing_edits() {
                    let owner = self.state.owner;
                    for handed in self.handed.clone() {
                        self.send(owner, &handed.message());
                    }
                    return;
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

    fn replacement(&mut self, head: CommitId, mut replaced: C) {
        let Role::Owner(sequencer) = &self.role else {
            return;
        };
        let unsealed: Vec<C::Op> = sequencer
            .since(0)
            .iter()
            .filter_map(|op| C::decode_operation(&op.payload).ok())
            .collect();
        for operation in &unsealed {
            replaced.apply(operation);
        }
        self.confirmed = replaced;
        self.replace_visible(self.confirmed.clone());
        self.base = Some(head);
        self.reload = true;
        self.op_heads.clear();
        match unsealed.is_empty() {
            true => self.settle_at(head),
            false => self.jobs.push_front(Job::Seal {
                sealing: Sealing::Once,
                reply: None,
            }),
        }
    }

    fn rebuild_visible(&mut self) {
        let mut visible = self.confirmed.clone();
        if let Role::Follower(follower) = &self.role {
            for payload in follower.payloads() {
                if let Ok(pending) = C::decode_operation(&payload) {
                    visible.apply(&pending);
                }
            }
        }
        for (operation, ..) in &self.unsent {
            visible.apply(operation);
        }
        self.replace_visible(visible);
    }

    fn flush(&mut self) {
        let Role::Follower(follower) = &mut self.role else {
            return;
        };
        if follower.pending() > 0 || self.awaiting.is_some() {
            return;
        }
        if self
            .unsent
            .front()
            .is_some_and(|(_, bound, _)| *bound > LARGEST_RELAYED_OPERATION)
        {
            return self.hand_over_oversized();
        }
        let Some((operation, _, weight)) = self.unsent.pop_front() else {
            return;
        };
        let message = follower.submit(C::encode_operation(&operation));
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
        let Ok(operation) = C::decode_operation(&op.payload) else {
            return self.unreadable_message();
        };
        if inherited == Inherited::Continued {
            self.append_unappended();
            self.append(vec![op.clone()], false);
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
        let Ok(operation) = C::decode_operation(&op.payload) else {
            return self.unreadable_message();
        };
        self.applied_ops.push(op.clone());
        let Role::Follower(follower) = &mut self.role else {
            return;
        };
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
        follower.rewrite_pending(|payload| {
            let pending = C::decode_operation(payload).ok()?;
            C::rebase(pending, std::slice::from_ref(&operation))
                .map(|rebased| C::encode_operation(&rebased))
        });
        if !self.covered {
            self.confirmed.apply(&operation);
            self.rebuild_visible();
            return;
        }
        let mut local: Vec<C::Op> = follower
            .payloads()
            .iter()
            .filter_map(|payload| C::decode_operation(payload).ok())
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
                return;
            }
            Role::Follower(follower) => follower,
        };
        let replacement = self.awaiting.take().filter(|_| self.replacing);
        self.replacing = false;
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
                self.greet();
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
        if std::mem::take(&mut self.catching_up) {
            sequencer.carry_unsealed();
            self.reload = true;
        }
        let mut accepted = Vec::new();
        for (id, payload) in pending {
            let Ok(operation) = C::decode_operation(&payload) else {
                continue;
            };
            if let Some(op) = sequencer.accept(id, payload) {
                self.confirmed.apply(&operation);
                accepted.push(op);
            }
        }
        self.in_flight = 0;
        let mut oversized = false;
        for (operation, bound, _) in std::mem::take(&mut self.unsent) {
            if bound > LARGEST_RELAYED_OPERATION {
                self.confirmed.apply(&operation);
                oversized = true;
                continue;
            }
            let id = OpId {
                client: self.client,
                counter: sequencer.sequence() + 1,
            };
            if let Some(op) = sequencer.accept(id, C::encode_operation(&operation)) {
                self.confirmed.apply(&operation);
                accepted.push(op);
            }
        }
        if oversized {
            self.hand_over_oversized();
        }
        let replays = sequencer.inherited() == 0 && !self.handing_edits() && !oversized;
        self.op_heads = match replays {
            true => vec![self.base],
            false => Vec::new(),
        };
        let inherited = sequencer.inherited() > 0;
        self.role = Role::Owner(sequencer);
        self.rebased = false;
        self.appended_bytes = 0;
        self.append(kept, false);
        self.replace_visible(self.confirmed.clone());
        for handed in self.handed.clone() {
            self.jobs.push_back(Job::Keep(Keeping {
                commit: handed.commit,
                base: handed.base,
                edits: None,
                from: self.client,
                attempts: 0,
            }));
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

    fn moved(&mut self, head: CommitId, from_this_device: bool) {
        if self.base == Some(head) {
            return;
        }
        if self.is_owner() {
            return self.jobs.push_front(Job::Reconcile);
        }
        if from_this_device {
            let owner = self.state.owner;
            self.send(owner, &SessionMessage::Moved { head });
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
        let Role::Owner(sequencer) = &self.role else {
            return;
        };
        if self.saved.contains(&keeping.commit) {
            return self.acknowledge(&keeping);
        }
        let Some(edits) = keeping
            .edits
            .filter(|edits| edits.applied >= self.replaced_at)
        else {
            return self.publish_kept(keeping, None);
        };
        let replay = sequencer.inherited() == 0
            && edits.applied <= sequencer.sequence()
            && sequencer.sequence() - sequencer.pending_operations() as u64 <= edits.applied
            && self.op_heads.contains(&edits.sealed);
        let commits = match replay {
            true => vec![Some(keeping.commit)],
            false => vec![keeping.base, Some(keeping.commit)],
        };
        self.read(commits, Then::Handed { keeping, replay });
    }

    fn hand_over_oversized(&mut self) {
        if !self.jobs.iter().any(|job| matches!(job, Job::Oversized)) {
            self.jobs.push_front(Job::Oversized);
        }
    }

    fn oversized(&mut self) {
        let Role::Follower(follower) = &self.role else {
            return self.replace(self.visible.clone(), None, false);
        };
        if follower.pending() > 0 || self.awaiting.is_some() || self.unsent.is_empty() {
            return;
        }
        if self.diverged {
            return self.error(DIVERGED);
        }
        let applied = follower.applied();
        let content = self.visible.clone();
        if applied == self.sealed && !self.handing_edits() {
            let written = self.written(&content);
            return self.stash(
                self.base,
                written,
                AfterStash::Hand {
                    content,
                    applied,
                    ancestor: self.base,
                },
            );
        }
        let ancestor = self.written(&self.confirmed);
        self.stash(
            self.base,
            ancestor,
            AfterStash::Ancestor { content, applied },
        );
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
        self.replace_visible(self.confirmed.clone());
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
        self.restarting = self.restart_due();
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
        sequencer.sealed(head);
        let sequence = sequencer.sequence();
        self.sealed = sequence;
        self.sealed_state = self.confirmed.session_state();
        self.owes_since = None;
        self.unappended.clear();
        self.append_at = None;
        self.appended_bytes = 0;
        let restart = std::mem::take(&mut self.restarting);
        let reload = match restart {
            true => {
                self.reload = false;
                Some(Vec::new())
            }
            false => {
                self.rebase_journal();
                std::mem::take(&mut self.reload).then(|| self.sealed_state.clone())
            }
        };
        if reload.is_some() {
            self.op_heads.clear();
        }
        if self.op_heads.len() >= OP_HEADS {
            self.op_heads.remove(0);
        }
        self.op_heads.push(Some(head));
        self.broadcast(&SessionMessage::Sealed {
            head,
            sequence,
            reload,
            restart,
        });
        self.beaten = Some(self.now);
        self.active = false;
        self.out.push(Effect::ReportClean {
            replica: self.id,
            generation: self.state.generation,
            clean_at: Some(head),
        });
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
        let mut carried = Vec::new();
        for held in std::mem::take(&mut self.held) {
            let moved = C::carry_over(&held, &before, &after, self.client);
            before.apply(&held);
            if let Some(moved) = moved {
                after.apply(&moved);
                carried.push(moved);
            }
        }
        self.held = carried;
        self.confirmed = fresh;
        self.sealed_state = self.confirmed.session_state();
        let mut visible = self.confirmed.clone();
        for held in &self.held {
            visible.apply(held);
        }
        self.replace_visible(visible);
        self.history.restart();
        self.restarted_at = Some(head);
        self.op_heads = vec![Some(head)];
        self.rebase_journal();
    }

    /// Moves a follower's pending and unsent edits from `before` onto the content it just
    /// reloaded at a restart.
    fn carry_pending(&mut self, before: C) {
        let mut before = before;
        let mut after = self.confirmed.clone();
        if let Role::Follower(follower) = &mut self.role {
            let pending = follower.take_pending();
            let mut carried = Vec::with_capacity(pending.len());
            for (id, payload) in pending {
                let Ok(operation) = C::decode_operation(&payload) else {
                    continue;
                };
                let moved = C::carry_over(&operation, &before, &after, self.client);
                before.apply(&operation);
                if let Some(moved) = moved {
                    after.apply(&moved);
                    carried.push((id, C::encode_operation(&moved)));
                }
            }
            follower.restore_pending(carried);
        }
        let unsent = std::mem::take(&mut self.unsent);
        for (operation, bound, weight) in unsent {
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
        payload: C::encode_operation(&current.1),
        ..current.0
    };
    for op in ops {
        let Ok(operation) = C::decode_operation(&op.payload) else {
            folded.extend(current.take().map(finish));
            folded.push(op);
            continue;
        };
        let size = op.payload.len();
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
        let operation = C::decode_operation(&op.payload).map_err(|error| error.to_string())?;
        content.apply(&operation);
    }
    Ok(content.encode())
}
