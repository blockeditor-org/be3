use std::collections::{BTreeMap, BTreeSet, VecDeque};

use be_block::{Landed, LiveEdit, Merge, Settling};
use be_commit::{CommitId, MergeResult};
use be_protocol::{ClientId, SessionState};
use be_session::{
    Follower, OpId, Resume, SessionLog, SessionMessage, SessionOp, SessionSeal, Submission,
};
use block_type_api::{
    Change, Command, EditorId, Endpoint, HeadEvent, Merging, Recipients, ReplicaId, Reply,
    RequestId, SessionTag, Status, Token, Written,
};
use uuid::Uuid;

use crate::{
    FromFileFn, Recorder,
    editor::{EditorOperation, FromEditor, Gesture, PeerMessage, PeerPresence, Snapshot, ToEditor},
    wire::Entry,
};

pub const SEAL_INTERVAL: u64 = 750;

const SETTLING_PATIENCE: u64 = 5_000;
const JOURNAL_LIMIT: usize = 1024;
const LARGEST_RELAYED_OPERATION: usize = 1024 * 1024;
const RELOAD_ATTEMPTS: u8 = 3;
const DIVERGED: &str = "this session is held by another build of the block's type";

pub(crate) struct Opened {
    pub(crate) replica: ReplicaId,
    pub(crate) content_type: Uuid,
    pub(crate) organization: Uuid,
    pub(crate) client: ClientId,
    pub(crate) tag: SessionTag,
    pub(crate) head: Option<CommitId>,
    pub(crate) session: SessionState,
    pub(crate) from_file: Option<FromFileFn>,
    pub(crate) now: u64,
}

pub(crate) trait AnyReplica {
    fn start(&mut self, out: &mut Vec<Command>);

    fn input(&mut self, now: u64, input: Input, out: &mut Vec<Command>);

    fn tick(&mut self, now: u64, out: &mut Vec<Command>);

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
        client: ClientId,
        session: SessionState,
        resynchronize: bool,
    },
    Sequenced(SessionOp),
    Caught {
        token: Token,
        result: Result<SessionLog, String>,
    },
    Read {
        token: Token,
        result: Result<Vec<u8>, String>,
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
    },
    Oversized,
    Release,
}

enum Slot<C> {
    Waiting(Token),
    Ready(C),
}

struct Logged<Op> {
    sequence: u64,
    origin: u64,
    op: Option<Op>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Dropped {
    Floor,
    Rebase,
    Unreadable,
}

struct Reloading {
    id: OpId,
    head: bool,
    folded: usize,
    before: Option<CommitId>,
}

struct Catching {
    through: u64,
    counter: u64,
}

struct Sent {
    submission: Submission,
    barrier: bool,
}

enum Then {
    Caught {
        log: SessionLog,
    },
    Reload {
        op: SessionOp,
        commit: CommitId,
        head: bool,
        replace: bool,
        state: Option<Vec<u8>>,
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
    Offline {
        base: Option<CommitId>,
        ours: CommitId,
    },
}

enum AfterPublish<C> {
    Seal {
        retry: bool,
        reply: Option<RequestId>,
        kept: Option<CommitId>,
    },
    Merged {
        value: C,
    },
    Provisional {
        reply: Option<RequestId>,
    },
}

enum AfterStash<C> {
    Merge {
        value: C,
        expected: CommitId,
        base: Option<CommitId>,
        count: usize,
    },
    Unsaved {
        content: C,
        reply: Option<RequestId>,
    },
    Replacement {
        state: Vec<u8>,
        reply: Option<RequestId>,
    },
    Fold {
        folded: usize,
        state: Vec<u8>,
    },
    Keep,
}

enum Wait<C> {
    Reads { slots: Vec<Slot<C>>, then: Then },
    Resume { token: Token, offline: bool },
    Publish { token: Token, then: AfterPublish<C> },
    Stash { token: Token, then: AfterStash<C> },
    Catchup { token: Token },
}

#[derive(Default)]
struct Reported {
    status: Option<Status>,
    history: Option<(bool, bool)>,
}

pub(crate) struct Replica<C: LiveEdit, H> {
    id: ReplicaId,
    content_type: Uuid,
    organization: Uuid,
    client: ClientId,
    tag: SessionTag,
    from_file: Option<FromFileFn>,
    state: SessionState,
    follower: Follower,
    confirmed: C,
    visible: C,
    unsent: VecDeque<(C::Op, usize, u64)>,
    in_flight: u64,
    covered: bool,
    base: Option<CommitId>,
    sealed: u64,
    log: VecDeque<Logged<C::Op>>,
    ahead: BTreeMap<u64, SessionOp>,
    keep: Vec<CommitId>,
    sent: Vec<Sent>,
    reloading: Vec<Reloading>,
    replaced_at: u64,
    unreadable_reloads: u8,
    catching_up: bool,
    catching: Option<Catching>,
    offline: Option<CommitId>,
    latest: Option<CommitId>,
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
    projected: Settling<C>,
    landed: Vec<(Landed<C::Op>, Option<EditorId>)>,
    resnapshot: bool,
    described: bool,
    jobs: VecDeque<Job<C>>,
    waiting: Option<Wait<C>>,
    tokens: Token,
    now: u64,
    owes_since: Option<u64>,
    sealing: bool,
    claimed: Option<u64>,
    closing: bool,
    after_settling: Vec<(u64, Job<C>)>,
    loaded: bool,
    released: bool,
    reported: Reported,
    out: Vec<Command>,
}

impl<C, H> AnyReplica for Replica<C, H>
where
    C: LiveEdit + Merge + Clone + Default,
    H: Recorder<C>,
{
    fn start(&mut self, out: &mut Vec<Command>) {
        self.check_build();
        self.catch_up();
        self.run(out);
    }

    fn input(&mut self, now: u64, input: Input, out: &mut Vec<Command>) {
        self.now = self.now.max(now);
        match input {
            Input::Read { token, result } => self.answer_read(token, result),
            Input::Resumed { token, result } => self.answer_resume(token, result),
            Input::Caught { token, result } => self.answer_caught(token, result),
            Input::Head(
                event @ (HeadEvent::Published { .. }
                | HeadEvent::Rejected { .. }
                | HeadEvent::Stashed { .. }
                | HeadEvent::Failed { .. }),
            ) => self.answer_head(event),
            Input::Close if !self.loaded => {
                self.waiting = None;
                self.release();
            }
            input @ Input::Lease {
                resynchronize: true,
                ..
            } if matches!(self.waiting, Some(Wait::Catchup { .. })) => {
                self.waiting = None;
                self.jobs.push_front(Job::Input(input));
            }
            input => self.jobs.push_back(Job::Input(input)),
        }
        self.run(out);
    }

    fn tick(&mut self, now: u64, out: &mut Vec<Command>) {
        self.now = self.now.max(now);
        let due = self
            .owes_since
            .is_some_and(|since| self.now >= since + SEAL_INTERVAL);
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
        let seal = match self.sealing {
            true => None,
            false => self.owes_since.map(|since| since + SEAL_INTERVAL),
        };
        let settling = self
            .after_settling
            .first()
            .map(|(since, _)| since + SETTLING_PATIENCE);
        seal.into_iter().chain(settling).min()
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
        let origin = Uuid::new_v4().as_u64_pair().0;
        Self {
            id: opened.replica,
            content_type: opened.content_type,
            organization: opened.organization,
            client: opened.client,
            tag: opened.tag,
            from_file: opened.from_file,
            state: opened.session,
            follower: Follower::new(origin),
            projected: Settling::new(empty.clone(), Vec::new()),
            visible: empty.clone(),
            confirmed: empty,
            unsent: VecDeque::new(),
            in_flight: 0,
            covered: true,
            base: opened.head,
            sealed: 0,
            log: VecDeque::new(),
            ahead: BTreeMap::new(),
            keep: Vec::new(),
            sent: Vec::new(),
            reloading: Vec::new(),
            replaced_at: 0,
            unreadable_reloads: 0,
            catching_up: true,
            catching: None,
            offline: opened.head,
            latest: None,
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
            landed: Vec::new(),
            resnapshot: false,
            described: false,
            jobs: VecDeque::new(),
            waiting: None,
            tokens: 0,
            now: opened.now,
            owes_since: None,
            sealing: false,
            claimed: None,
            closing: false,
            after_settling: Vec::new(),
            loaded: false,
            released: false,
            reported: Reported::default(),
            out: Vec::new(),
        }
    }

    fn run(&mut self, out: &mut Vec<Command>) {
        loop {
            self.flush_editors();
            if self.waiting.is_some() || self.released {
                break;
            }
            if self.drain() {
                continue;
            }
            if let Some(job) = self.jobs.pop_front() {
                self.job(job);
                continue;
            }
            if self.run_settled() {
                continue;
            }
            break;
        }
        self.report();
        out.append(&mut self.out);
    }

    fn job(&mut self, job: Job<C>) {
        if self.unreadable.is_some() {
            return self.job_while_unreadable(job);
        }
        match job {
            Job::Input(input) => self.handle(input),
            Job::Seal { sealing, reply } => self.seal(sealing, reply),
            Job::Reconcile => self.reconcile(),
            Job::Replace { content, reply } => self.replace(content, reply),
            Job::Oversized => self.oversized(),
            Job::Release => self.release(),
        }
    }

    fn job_while_unreadable(&mut self, job: Job<C>) {
        match job {
            Job::Input(Input::Close) | Job::Release => self.release(),
            Job::Input(Input::Change { id, .. }) => self.reply(id, Reply::Changed(false)),
            Job::Input(Input::Content { id }) | Job::Input(Input::Seal { id, .. }) => {
                self.reply(id, Reply::Unknown);
            }
            Job::Seal {
                reply: Some(id), ..
            }
            | Job::Replace {
                reply: Some(id), ..
            } => self.reply(id, Reply::Unknown),
            _ => {}
        }
    }

    fn release(&mut self) {
        if self.released {
            return;
        }
        self.out.push(Command::Release { replica: self.id });
        self.released = true;
        let settling = std::mem::take(&mut self.after_settling)
            .into_iter()
            .map(|(_, job)| job);
        for job in std::mem::take(&mut self.jobs).into_iter().chain(settling) {
            self.job_while_unreadable(job);
        }
    }

    fn run_settled(&mut self) -> bool {
        let Some((since, _)) = self.after_settling.first() else {
            return false;
        };
        let settled = self.unreadable.is_some()
            || (self.loaded
                && !self.catching_up
                && self.follower.pending() == 0
                && self.unsent.is_empty()
                && self.reloading.is_empty());
        if !settled && self.now < since + SETTLING_PATIENCE {
            return false;
        }
        for (_, job) in std::mem::take(&mut self.after_settling) {
            self.jobs.push_back(job);
        }
        true
    }

    fn handle(&mut self, input: Input) {
        match input {
            Input::Message {
                from: Endpoint::Peer(from),
                bytes,
            } => match SessionMessage::decode(&bytes) {
                Some(message) => self.receive(from, message),
                None => self.incompatible = true,
            },
            Input::Message {
                from: Endpoint::Editor(editor),
                bytes,
            } => {
                if let Some(message) = FromEditor::decode(&bytes) {
                    self.handle_editor(editor, message);
                }
            }
            Input::Message {
                from: Endpoint::Host,
                ..
            } => {}
            Input::Attach { editor, editable } => self.attach(editor, editable),
            Input::Detach { editor } => {
                self.editors.remove(&editor);
            }
            Input::Head(HeadEvent::Moved { head, .. }) => self.moved(head),
            Input::Head(_) | Input::Read { .. } | Input::Resumed { .. } | Input::Caught { .. } => {}
            Input::Lease {
                client,
                session,
                resynchronize,
            } => {
                let rejoined = client != self.client;
                self.client = client;
                self.adopt(session);
                if rejoined {
                    for (kind, value) in self.shown.clone() {
                        self.broadcast(&SessionMessage::Presence {
                            kind,
                            value: Some(value),
                        });
                    }
                }
                match resynchronize {
                    true => self.catch_up(),
                    false => self.flush(),
                }
            }
            Input::Sequenced(op) => {
                if op.sequence > self.follower.applied() {
                    self.ahead.insert(op.sequence, op);
                }
            }
            Input::Claim => self.claim(),
            Input::Close => {
                if !std::mem::replace(&mut self.closing, true) {
                    let sealing = Job::Seal {
                        sealing: Sealing::Due,
                        reply: None,
                    };
                    self.after_settling.push((self.now, sealing));
                    self.after_settling.push((self.now, Job::Release));
                }
            }
            Input::Change { id, change } => self.change(id, change),
            Input::Content { id } => {
                let bytes = self.visible.encode();
                self.reply(id, Reply::Content(bytes));
            }
            Input::Seal { id, force } => {
                let sealing = Job::Seal {
                    sealing: match force {
                        true => Sealing::Rekey,
                        false => Sealing::Due,
                    },
                    reply: Some(id),
                };
                self.jobs.push_front(sealing);
            }
        }
    }

    fn reply(&mut self, id: RequestId, reply: Reply) {
        self.out.push(Command::Reply { id, reply });
    }

    fn error(&mut self, message: impl Into<String>) {
        self.out.push(Command::Error {
            replica: self.id,
            message: message.into(),
        });
    }

    fn token(&mut self) -> Token {
        self.tokens += 1;
        self.tokens
    }

    fn written(&self, content: &C) -> Written {
        Written {
            bytes: content.encode(),
            references: content.references_in(self.organization),
            derived: be_block::derived_metadata(content),
        }
    }

    fn is_holder(&self) -> bool {
        self.state.is_owner(self.client)
    }

    fn owes(&self) -> bool {
        self.loaded
            && !self.catching_up
            && self.is_holder()
            && (self.follower.applied() > self.sealed || !self.keep.is_empty())
    }

    fn is_clean(&self) -> bool {
        self.follower.pending() == 0 && self.unsent.is_empty() && !self.owes()
    }

    fn has_unsaved(&self) -> bool {
        self.follower.pending() > 0
            || !self.unsent.is_empty()
            || self.follower.applied() > self.sealed
    }

    fn check_build(&mut self) {
        if self.state.tag.is_some_and(|tag| tag != self.tag.0) {
            self.incompatible = true;
            self.diverged = true;
        }
    }

    fn catch_up(&mut self) {
        self.catching_up = true;
        self.catching = None;
        let token = self.token();
        self.out.push(Command::Catchup {
            replica: self.id,
            token,
            origin: self.follower.origin(),
        });
        self.waiting = Some(Wait::Catchup { token });
    }

    fn answer_caught(&mut self, token: Token, result: Result<SessionLog, String>) {
        if !matches!(&self.waiting, Some(Wait::Catchup { token: waiting }) if *waiting == token) {
            return;
        }
        self.waiting = None;
        match result {
            Ok(log) => self.caught(log),
            Err(error) => self.error(error),
        }
    }

    fn caught(&mut self, log: SessionLog) {
        if !self.loaded || self.follower.applied() < log.seal.sequence {
            let head = log.seal.head;
            return self.read(vec![head], Then::Caught { log });
        }
        self.take_log(log, false);
    }

    fn take_log(&mut self, log: SessionLog, context: bool) {
        let applied = self.follower.applied();
        let through = log
            .ops
            .last()
            .map_or(applied, |op| op.sequence)
            .max(applied);
        self.catching = Some(Catching {
            through,
            counter: log.counter,
        });
        for op in log.ops {
            match op.sequence <= applied {
                true if context => self.context(op),
                true => {}
                false => {
                    self.ahead.insert(op.sequence, op);
                }
            }
        }
    }

    fn context(&mut self, op: SessionOp) {
        self.trim_log(op.floor);
        let rebased = match Entry::decode(&op.payload) {
            Some(Entry::Operation(bytes)) if op.base >= op.floor => C::decode_operation(&bytes)
                .ok()
                .and_then(|operation| self.rebased(operation, op.base, op.sequence)),
            _ => None,
        };
        self.log.push_back(Logged {
            sequence: op.sequence,
            origin: op.id.origin,
            op: rebased,
        });
    }

    fn finish_catchup(&mut self) {
        let Some(catching) = self.catching.take() else {
            return;
        };
        self.catching_up = false;
        let counter = catching.counter;
        let pending = self.follower.pending();
        self.follower.sequenced_through(counter);
        self.sent
            .retain(|sent| sent.submission.id.counter > counter);
        let (trimmed, reloading): (Vec<Reloading>, Vec<Reloading>) =
            std::mem::take(&mut self.reloading)
                .into_iter()
                .partition(|reloading| reloading.id.counter <= counter);
        self.reloading = reloading;
        for reloading in trimmed {
            for _ in 0..reloading.folded {
                self.unsent.pop_front();
            }
        }
        if self.follower.pending() == 0 {
            self.in_flight = 0;
        }
        let mut resubmit: Vec<(Submission, bool)> = self
            .follower
            .resynchronize()
            .into_iter()
            .map(|submission| Submission {
                payload: Entry::Operation(submission.payload).encode(),
                ..submission
            })
            .map(|submission| (submission, false))
            .chain(
                self.sent
                    .iter()
                    .map(|sent| (sent.submission.clone(), sent.barrier)),
            )
            .collect();
        resubmit.sort_by_key(|(submission, _)| submission.id.counter);
        for (submission, barrier) in resubmit {
            self.submit(submission, barrier);
        }
        if self.follower.pending() != pending {
            self.rebuild_visible();
        }
        self.cover_if_settled();
        self.claim_if_abandoned();
        self.reconcile_if_behind();
        self.flush();
    }

    fn trim_log(&mut self, floor: u64) {
        while self.log.front().is_some_and(|held| held.sequence <= floor) {
            self.log.pop_front();
        }
    }

    fn rebased(&self, operation: C::Op, base: u64, sequence: u64) -> Option<C::Op> {
        let onto: Vec<C::Op> = self
            .log
            .iter()
            .filter(|held| held.sequence > base && held.sequence < sequence)
            .filter_map(|held| held.op.clone())
            .collect();
        match onto.is_empty() {
            true => Some(operation),
            false => C::rebase(operation, &onto),
        }
    }

    fn drain(&mut self) -> bool {
        if self.waiting.is_some() || self.unreadable.is_some() || !self.loaded {
            return false;
        }
        let applied = self.follower.applied();
        if let Some(op) = self.ahead.remove(&(applied + 1)) {
            self.sequenced(op);
            if self.waiting.is_none() {
                self.flush();
            }
            return true;
        }
        self.ahead.retain(|sequence, _| *sequence > applied);
        if self
            .catching
            .as_ref()
            .is_some_and(|catching| applied >= catching.through)
        {
            self.finish_catchup();
            return true;
        }
        false
    }

    fn passed(&mut self, op: &SessionOp) {
        self.log.push_back(Logged {
            sequence: op.sequence,
            origin: op.id.origin,
            op: None,
        });
        self.follower.set_applied(op.sequence);
    }

    fn sequenced(&mut self, op: SessionOp) {
        self.trim_log(op.floor);
        let mine = self.follower.is_mine(op.id);
        if mine {
            self.sent.retain(|sent| sent.submission.id != op.id);
        }
        match Entry::decode(&op.payload) {
            Some(Entry::Operation(bytes)) => self.sequenced_operation(op, &bytes),
            Some(Entry::Reload {
                commit,
                head,
                replace,
                state,
            }) => {
                if replace || op.base >= op.floor {
                    return self.read(
                        vec![Some(commit)],
                        Then::Reload {
                            op,
                            commit,
                            head,
                            replace,
                            state,
                        },
                    );
                }
                self.passed(&op);
                if mine {
                    self.reload_dropped(op.id);
                }
            }
            Some(Entry::Keep { commit }) => {
                self.passed(&op);
                if !self.keep.contains(&commit) {
                    self.keep.push(commit);
                }
                self.seal_if_owed();
            }
            None => {
                self.passed(&op);
                self.incompatible = true;
                if mine && self.follower.forget(op.id).is_some() {
                    self.in_flight = 0;
                    self.rebuild_visible();
                    self.cover_if_settled();
                }
            }
        }
    }

    fn sequenced_operation(&mut self, op: SessionOp, bytes: &[u8]) {
        let outcome = match op.base < op.floor {
            true => Err(Dropped::Floor),
            false => match C::decode_operation(bytes) {
                Err(_) => Err(Dropped::Unreadable),
                Ok(operation) => self
                    .rebased(operation, op.base, op.sequence)
                    .ok_or(Dropped::Rebase),
            },
        };
        self.log.push_back(Logged {
            sequence: op.sequence,
            origin: op.id.origin,
            op: outcome.as_ref().ok().cloned(),
        });
        let dropped = match outcome {
            Ok(operation) => return self.apply_sequenced(op.id, op.sequence, operation),
            Err(dropped) => dropped,
        };
        self.follower.set_applied(op.sequence);
        if dropped == Dropped::Unreadable {
            self.incompatible = true;
        }
        let Some(payload) = self.follower.forget(op.id) else {
            return;
        };
        let weight = std::mem::take(&mut self.in_flight).max(1);
        if dropped == Dropped::Floor
            && op.base >= self.replaced_at
            && let Ok(operation) = C::decode_operation(&payload)
        {
            self.unsent.push_front((operation, payload.len(), weight));
            return;
        }
        self.rebuild_visible();
        self.cover_if_settled();
    }

    fn apply_sequenced(&mut self, id: OpId, sequence: u64, operation: C::Op) {
        let mine = self.follower.is_mine(id);
        let unchanged = mine
            && self.follower.front() == Some(id)
            && self
                .follower
                .payloads()
                .first()
                .is_some_and(|payload| *payload == C::encode_operation(&operation));
        if unchanged {
            self.follower.accepted(id, sequence);
            self.confirmed.apply(&operation);
            self.in_flight = 0;
            self.cover_if_settled();
            return;
        }
        if !mine && self.follower.pending() == 0 && self.unsent.is_empty() {
            self.follower.accepted(id, sequence);
            self.confirmed.apply(&operation);
            self.visible.apply(&operation);
            self.journal(Journaled::Applied(operation), None);
            return;
        }
        if mine {
            self.follower.accepted(id, sequence);
            self.confirmed.apply(&operation);
            self.in_flight = 0;
            self.rebuild_visible();
            self.cover_if_settled();
            return;
        }
        let unsettled = self.unsettled();
        self.follower.accepted(id, sequence);
        self.follower.rewrite_pending(|payload| {
            let pending = C::decode_operation(payload).ok()?;
            C::rebase(pending, std::slice::from_ref(&operation))
                .map(|rebased| C::encode_operation(&rebased))
        });
        if !self.covered {
            self.confirmed.apply(&operation);
            self.rebuild_visible();
            return;
        }
        let mut local: Vec<C::Op> = self
            .follower
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

    fn loses_work_after(&self, base: u64) -> bool {
        let origin = self.follower.origin();
        self.follower.pending() > 0
            || !self.unsent.is_empty()
            || self
                .log
                .iter()
                .any(|held| held.sequence > base && held.origin == origin && held.op.is_some())
    }

    fn reloaded(&mut self, op: SessionOp, commit: CommitId, head: bool, replace: bool, content: C) {
        let mut content = content;
        let mine = self.follower.is_mine(op.id);
        let before =
            (replace && !mine && self.loses_work_after(op.base)).then(|| self.visible.clone());
        let mut reapplied = false;
        if !replace {
            for held in self.log.iter().filter(|held| held.sequence > op.base) {
                if let Some(operation) = &held.op {
                    content.apply(operation);
                    reapplied = true;
                }
            }
        }
        self.passed(&op);
        self.confirmed = content;
        if replace {
            self.replaced_at = op.sequence;
            self.history.clear();
            self.unsent.clear();
            self.follower.take_pending();
            self.in_flight = 0;
            self.covered = true;
            for reloading in &mut self.reloading {
                reloading.folded = 0;
            }
        }
        if mine && let Some(index) = self.reloading.iter().position(|held| held.id == op.id) {
            let reloading = self.reloading.remove(index);
            for _ in 0..reloading.folded {
                self.unsent.pop_front();
            }
        }
        match head {
            true => {
                self.base = Some(commit);
                if !reapplied {
                    self.sealed = op.sequence;
                }
            }
            false => {
                if !self.keep.contains(&commit) {
                    self.keep.push(commit);
                }
            }
        }
        self.rebuild_visible();
        self.cover_if_settled();
        if mine && head {
            self.reconcile_if_behind();
        }
        self.seal_if_owed();
        if let Some(before) = before {
            let written = self.written(&before);
            self.stash(self.base, written, AfterStash::Keep);
        }
    }

    fn reload_dropped(&mut self, id: OpId) {
        let Some(index) = self.reloading.iter().position(|held| held.id == id) else {
            return;
        };
        let reloading = self.reloading.remove(index);
        if reloading.head {
            self.base = reloading.before;
            self.jobs.push_back(Job::Reconcile);
        } else if reloading.folded > 0 {
            self.hand_over_oversized();
        }
    }

    fn seal_if_owed(&mut self) {
        let queued = self.jobs.iter().any(|job| {
            matches!(
                job,
                Job::Seal {
                    sealing: Sealing::Once,
                    ..
                }
            )
        });
        if self.owes() && !queued {
            self.jobs.push_front(Job::Seal {
                sealing: Sealing::Once,
                reply: None,
            });
        }
    }

    fn submit_operation(&mut self, submission: Submission) {
        let payload = Entry::Operation(submission.payload).encode();
        self.submit(
            Submission {
                payload,
                ..submission
            },
            false,
        );
    }

    fn submit(&mut self, submission: Submission, barrier: bool) {
        self.out.push(Command::Submit {
            replica: self.id,
            id: submission.id,
            base: submission.base,
            barrier,
            payload: submission.payload,
        });
    }

    fn submit_entry(&mut self, entry: Entry, barrier: bool) -> OpId {
        let submission = Submission {
            id: self.follower.next_id(),
            base: self.follower.applied(),
            payload: entry.encode(),
        };
        let id = submission.id;
        self.sent.push(Sent {
            submission: submission.clone(),
            barrier,
        });
        self.submit(submission, barrier);
        id
    }

    fn read(&mut self, commits: Vec<Option<CommitId>>, then: Then) {
        let slots = commits
            .into_iter()
            .map(|commit| match commit {
                Some(commit) => {
                    let token = self.token();
                    self.out.push(Command::Read {
                        replica: self.id,
                        token,
                        commit,
                        range: None,
                    });
                    Slot::Waiting(token)
                }
                None => Slot::Ready(C::default()),
            })
            .collect();
        self.waiting = Some(Wait::Reads { slots, then });
        self.finish_reads();
    }

    fn answer_read(&mut self, token: Token, result: Result<Vec<u8>, String>) {
        let Some(Wait::Reads { slots, .. }) = &mut self.waiting else {
            return;
        };
        let Some(slot) = slots
            .iter_mut()
            .find(|slot| matches!(slot, Slot::Waiting(waiting) if *waiting == token))
        else {
            return;
        };
        let decoded = result.and_then(|bytes| C::decode(&bytes).map_err(|error| error.to_string()));
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
                    Then::Caught { .. } if !self.loaded => self.unreadable = Some(error),
                    Then::Reload { op, .. } if self.unreadable_reloads < RELOAD_ATTEMPTS => {
                        self.unreadable_reloads += 1;
                        self.ahead.insert(op.sequence, op);
                        self.catch_up();
                    }
                    Then::Reload { op, .. } => {
                        self.error(error);
                        self.passed(&op);
                        self.diverged = true;
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
            Then::Caught { log } => {
                let mut content = next();
                if let Some(state) = &log.seal.state
                    && content.adopt_session_state(state).is_err()
                {
                    self.incompatible = true;
                }
                self.confirmed = content;
                self.base = log.seal.head;
                self.sealed = log.seal.sequence;
                self.follower.set_applied(log.seal.sequence);
                self.log.clear();
                let sealed = log.seal.sequence;
                self.ahead.retain(|sequence, _| *sequence > sealed);
                match self.loaded {
                    true => self.rebuild_visible(),
                    false => self.loaded(),
                }
                self.take_log(log, true);
            }
            Then::Reload {
                op,
                commit,
                head,
                replace,
                state,
            } => {
                self.unreadable_reloads = 0;
                let mut content = next();
                if let Some(state) = &state
                    && content.adopt_session_state(state).is_err()
                {
                    self.incompatible = true;
                }
                self.reloaded(op, commit, head, replace, content);
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
            Then::Offline { base, ours } => {
                let ancestor = next();
                let ours_content = next();
                let (value, conflicts) = match C::merge3(&ancestor, &ours_content, &self.confirmed)
                {
                    MergeResult::Clean(value) => (value, None),
                    MergeResult::Conflicted { value, conflicts } => (value, Some(conflicts)),
                };
                if let (Some(count), Some(theirs)) = (conflicts, self.base) {
                    self.conflict(ours, theirs, count);
                }
                let content = self.written(&value);
                self.publish(
                    self.base,
                    Some(Merging { base, ours }),
                    content,
                    AfterPublish::Merged { value },
                );
            }
        }
    }

    fn conflict(&mut self, ours: CommitId, theirs: CommitId, count: usize) {
        self.out.push(Command::Conflict {
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
        self.out.push(Command::Publish {
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
        self.out.push(Command::Stash {
            replica: self.id,
            token,
            parent,
            content,
        });
        self.waiting = Some(Wait::Stash { token, then });
    }

    fn answer_resume(&mut self, token: Token, result: Result<Resume, String>) {
        let offline = match &self.waiting {
            Some(Wait::Resume {
                token: waiting,
                offline,
            }) if *waiting == token => *offline,
            _ => return,
        };
        self.waiting = None;
        let resume = match result {
            Ok(resume) => resume,
            Err(error) => return self.error(error),
        };
        if offline {
            match resume {
                Resume::Merge { base, ours, .. } => {
                    self.read(vec![base, Some(ours)], Then::Offline { base, ours });
                }
                Resume::Publish { from } => {
                    let base = self.base;
                    self.read(vec![base, Some(from)], Then::Offline { base, ours: from });
                }
                Resume::Empty | Resume::UpToDate | Resume::FastForward { .. } => {}
            }
            return;
        }
        let unsealed = self.follower.applied() > self.sealed;
        match resume {
            Resume::Empty | Resume::UpToDate | Resume::Publish { .. } => {}
            Resume::FastForward { to } if !unsealed => self.reload_head(to, None),
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
        match (self.waiting.take(), event) {
            (Some(Wait::Publish { then, .. }), HeadEvent::Published { head, .. }) => {
                self.published(then, head);
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
                        then: AfterPublish::Seal { reply, .. } | AfterPublish::Provisional { reply },
                        ..
                    }) => {
                        self.sealing = false;
                        reply.map(|id| (id, Reply::Sealed))
                    }
                    Some(Wait::Stash {
                        then:
                            AfterStash::Unsaved { reply, .. } | AfterStash::Replacement { reply, .. },
                        ..
                    }) => reply.map(|id| (id, Reply::Changed(false))),
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

    fn published(&mut self, then: AfterPublish<C>, head: CommitId) {
        match then {
            AfterPublish::Seal { reply, kept, .. } => {
                self.settle_at(head, kept);
                self.sealing = false;
                if let Some(id) = reply {
                    self.reply(id, Reply::Sealed);
                }
            }
            AfterPublish::Merged { value } => {
                let state = value.session_state();
                self.reload_head(head, Some(state));
            }
            AfterPublish::Provisional { reply } => {
                self.base = Some(head);
                self.sealing = false;
                if let Some(id) = reply {
                    self.reply(id, Reply::Sealed);
                }
            }
        }
    }

    fn rejected(&mut self, then: AfterPublish<C>, head: Option<CommitId>) {
        self.latest = head;
        match then {
            AfterPublish::Seal {
                retry: true, reply, ..
            } => {
                self.jobs.push_front(Job::Seal {
                    sealing: Sealing::Once,
                    reply,
                });
                self.jobs.push_front(Job::Reconcile);
            }
            AfterPublish::Seal {
                retry: false,
                reply,
                ..
            } => {
                self.sealing = false;
                if let Some(id) = reply {
                    self.reply(id, Reply::Sealed);
                }
            }
            AfterPublish::Merged { .. } => {}
            AfterPublish::Provisional { reply } => {
                self.sealing = false;
                if let Some(id) = reply {
                    self.reply(id, Reply::Sealed);
                }
                self.jobs.push_front(Job::Reconcile);
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
            AfterStash::Unsaved { content, reply } => {
                let written = self.written(&content);
                let state = content.session_state();
                self.stash(
                    Some(commit),
                    written,
                    AfterStash::Replacement { state, reply },
                );
            }
            AfterStash::Replacement { state, reply } => {
                self.submit_entry(
                    Entry::Reload {
                        commit,
                        head: false,
                        replace: true,
                        state: Some(state),
                    },
                    true,
                );
                if let Some(id) = reply {
                    self.reply(id, Reply::Changed(true));
                }
            }
            AfterStash::Fold { folded, state } => {
                let id = self.submit_entry(
                    Entry::Reload {
                        commit,
                        head: false,
                        replace: false,
                        state: Some(state),
                    },
                    false,
                );
                self.reloading.push(Reloading {
                    id,
                    head: false,
                    folded,
                    before: None,
                });
            }
            AfterStash::Keep => {
                self.submit_entry(Entry::Keep { commit }, false);
            }
        }
    }

    fn reload_head(&mut self, head: CommitId, state: Option<Vec<u8>>) {
        let before = self.base;
        self.base = Some(head);
        let id = self.submit_entry(
            Entry::Reload {
                commit: head,
                head: true,
                replace: false,
                state,
            },
            false,
        );
        self.reloading.push(Reloading {
            id,
            head: true,
            folded: 0,
            before,
        });
    }

    fn loaded(&mut self) {
        self.visible = self.confirmed.clone();
        for (operation, ..) in &self.unsent {
            self.visible.apply(operation);
        }
        self.projected = Settling::new(self.visible.clone(), Vec::new());
        self.loaded = true;
        self.described = true;
        if self.offline == self.base {
            self.offline = None;
        }
    }

    fn unsettled(&self) -> u64 {
        self.in_flight + self.unsent.iter().map(|(.., weight)| weight).sum::<u64>()
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

    fn rebuild_visible(&mut self) {
        let mut visible = self.confirmed.clone();
        for payload in self.follower.payloads() {
            if let Ok(pending) = C::decode_operation(&payload) {
                visible.apply(&pending);
            }
        }
        for (operation, ..) in &self.unsent {
            visible.apply(operation);
        }
        self.replace_visible(visible);
    }

    fn folded(&self) -> usize {
        self.reloading
            .iter()
            .map(|reloading| reloading.folded)
            .sum()
    }

    fn edit(&mut self, operation: C::Op, origin: Option<EditorId>) -> bool {
        if self.diverged {
            self.error(DIVERGED);
            return false;
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
        let payload = C::encode_operation(&operation);
        let size = payload.len();
        let oversized = size > LARGEST_RELAYED_OPERATION;
        let held = oversized
            || self.follower.pending() > 0
            || !self.unsent.is_empty()
            || !self.reloading.is_empty()
            || self.catching_up;
        if !held {
            let submission = self.follower.submit(payload);
            self.in_flight = 1;
            self.submit_operation(submission);
            return true;
        }
        let absorbable = self.unsent.len() > self.folded();
        let leftover = match self.unsent.back_mut() {
            Some((last, bound, weight))
                if absorbable && *bound + size <= LARGEST_RELAYED_OPERATION =>
            {
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
        true
    }

    fn flush(&mut self) {
        if self.follower.pending() > 0
            || !self.reloading.is_empty()
            || self.catching_up
            || !self.loaded
        {
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
        let submission = self.follower.submit(C::encode_operation(&operation));
        self.in_flight = weight;
        self.submit_operation(submission);
    }

    fn hand_over_oversized(&mut self) {
        if !self.jobs.iter().any(|job| matches!(job, Job::Oversized)) {
            self.jobs.push_front(Job::Oversized);
        }
    }

    fn oversized(&mut self) {
        if self.follower.pending() > 0
            || !self.reloading.is_empty()
            || self.unsent.is_empty()
            || !self.loaded
        {
            return;
        }
        if self.diverged {
            return self.error(DIVERGED);
        }
        let content = self.visible.clone();
        let folded = self.unsent.len();
        let written = self.written(&content);
        let state = content.session_state();
        self.stash(self.base, written, AfterStash::Fold { folded, state });
    }

    fn replace(&mut self, content: C, reply: Option<RequestId>) {
        if self.diverged || !self.loaded {
            if self.diverged {
                self.error(DIVERGED);
            }
            if let Some(id) = reply {
                self.reply(id, Reply::Changed(false));
            }
            return;
        }
        if self.has_unsaved() {
            let kept = self.written(&self.visible);
            return self.stash(self.base, kept, AfterStash::Unsaved { content, reply });
        }
        let written = self.written(&content);
        let state = content.session_state();
        self.stash(self.base, written, AfterStash::Replacement { state, reply });
    }

    fn seal(&mut self, sealing: Sealing, reply: Option<RequestId>) {
        let able = self.is_holder()
            && self.loaded
            && !self.catching_up
            && !self.reloading.iter().any(|reloading| reloading.head);
        let provisional =
            reply.is_some() && (self.follower.pending() > 0 || !self.unsent.is_empty());
        let wanted = match sealing {
            Sealing::Rekey => !self.diverged,
            Sealing::Due | Sealing::Once => self.owes() || provisional,
        };
        if !able || !wanted || self.diverged {
            if able && wanted {
                self.error(DIVERGED);
            }
            self.sealing = false;
            if let Some(id) = reply {
                self.reply(id, Reply::Sealed);
            }
            return;
        }
        if provisional {
            let content = self.written(&self.visible);
            return self.publish(
                self.base,
                None,
                content,
                AfterPublish::Provisional { reply },
            );
        }
        let kept = self.keep.first().copied();
        let merging = kept.map(|ours| Merging {
            base: self.base,
            ours,
        });
        let content = self.written(&self.confirmed);
        self.publish(
            self.base,
            merging,
            content,
            AfterPublish::Seal {
                retry: sealing != Sealing::Once,
                reply,
                kept,
            },
        );
    }

    fn settle_at(&mut self, head: CommitId, kept: Option<CommitId>) {
        let sequence = self.follower.applied();
        self.base = Some(head);
        self.sealed = sequence;
        self.owes_since = None;
        if self.latest == Some(head) {
            self.latest = None;
        }
        if let Some(kept) = kept {
            self.keep.retain(|held| *held != kept);
        }
        self.broadcast(&SessionMessage::Sealed {
            head,
            sequence,
            kept,
        });
        self.out.push(Command::ReportClean {
            replica: self.id,
            generation: self.state.generation,
            seal: SessionSeal {
                head: Some(head),
                sequence,
                state: Some(self.confirmed.session_state()),
            },
        });
        if !self.keep.is_empty() {
            self.jobs.push_front(Job::Seal {
                sealing: Sealing::Once,
                reply: None,
            });
        }
    }

    fn reconcile(&mut self) {
        if !self.is_holder() || !self.loaded || self.catching_up {
            return;
        }
        let (local, offline) = match self.offline.take() {
            Some(offline) if Some(offline) != self.base => (Some(offline), true),
            _ => (self.base, false),
        };
        self.latest = None;
        let token = self.token();
        self.out.push(Command::Resume {
            replica: self.id,
            token,
            local,
        });
        self.waiting = Some(Wait::Resume { token, offline });
    }

    fn reconcile_if_behind(&mut self) {
        let behind =
            self.offline.is_some() || self.latest.is_some_and(|latest| Some(latest) != self.base);
        let queued = self.jobs.iter().any(|job| matches!(job, Job::Reconcile));
        if behind && !queued && self.is_holder() && self.loaded && !self.catching_up {
            self.jobs.push_back(Job::Reconcile);
        }
    }

    fn moved(&mut self, head: CommitId) {
        if self.base == Some(head) {
            return;
        }
        self.latest = Some(head);
        let reconciling = self.reloading.iter().any(|reloading| reloading.head);
        if self.is_holder() && self.loaded && !self.catching_up && !reconciling {
            self.jobs.push_front(Job::Reconcile);
        }
    }

    fn adopt(&mut self, state: SessionState) {
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
        let held = self.is_holder();
        self.state = state;
        self.check_build();
        for client in joined {
            self.show_presence_to(client);
        }
        self.claim_if_abandoned();
        if !held && self.is_holder() {
            self.reconcile_if_behind();
        }
    }

    fn claim(&mut self) {
        if !self.loaded || self.catching_up || self.is_holder() {
            return;
        }
        self.claimed = Some(self.state.generation);
        self.out.push(Command::ClaimLease {
            replica: self.id,
            generation: self.state.generation,
        });
    }

    fn claim_if_abandoned(&mut self) {
        if !self.loaded || self.catching_up {
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
        self.out.push(Command::ClaimLease {
            replica: self.id,
            generation: self.state.generation,
        });
    }

    fn receive(&mut self, from: ClientId, message: SessionMessage) {
        match message {
            SessionMessage::Sealed {
                head,
                sequence,
                kept,
            } => {
                if let Some(kept) = kept {
                    self.keep.retain(|held| *held != kept);
                }
                if sequence >= self.sealed {
                    self.base = Some(head);
                    self.sealed = sequence;
                    if self.latest == Some(head) {
                        self.latest = None;
                    }
                }
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
        }
    }

    fn report(&mut self) {
        let status = Status {
            head: self.base,
            owner: self.is_holder(),
            submitted: self.unsent.is_empty(),
            clean: self.is_clean(),
            owes_seal: self.owes(),
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
            self.out.push(Command::Status {
                replica: self.id,
                status,
            });
        }
        if !self.loaded || self.released {
            return;
        }
        let history = (self.history.can_undo(), self.history.can_redo());
        if self.reported.history != Some(history) {
            self.reported.history = Some(history);
            self.out.push(Command::History {
                replica: self.id,
                can_undo: history.0,
                can_redo: history.1,
            });
        }
        if std::mem::take(&mut self.described) {
            self.out.push(Command::Described {
                replica: self.id,
                description: crate::describe(&self.visible, self.organization),
            });
        }
    }

    fn send(&mut self, to: Option<ClientId>, message: &SessionMessage) {
        self.out.push(Command::Send {
            replica: self.id,
            to: match to {
                Some(client) => Recipients::One(Endpoint::Peer(client)),
                None => Recipients::Peers,
            },
            bytes: message.encode(),
        });
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
            FromEditor::Operate(_) | FromEditor::Gesture(_) | FromEditor::Replace(_) => {}
        }
    }

    fn change(&mut self, id: RequestId, change: Change) {
        match change {
            Change::Child(change) => {
                let Some(operations) = self.visible.child_operations(crate::be_child(change))
                else {
                    return self.reply(id, Reply::Changed(false));
                };
                for operation in operations {
                    self.record_and_edit(operation, None);
                }
                self.reply(id, Reply::Changed(true));
            }
            Change::FromFile { name, bytes } => {
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
        self.reply(id, Reply::Changed(changed));
    }

    fn replace_with(&mut self, id: RequestId, content: Option<C>) {
        let Some(content) = content else {
            return self.reply(id, Reply::Changed(false));
        };
        self.history.clear();
        self.jobs.push_front(Job::Replace {
            content,
            reply: Some(id),
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
        let snapshot = Snapshot {
            content_type: self.content_type,
            bytes: settled.encode(),
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
        self.out.push(Command::Send {
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
}
