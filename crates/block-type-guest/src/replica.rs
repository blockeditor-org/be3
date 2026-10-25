use std::collections::{BTreeMap, BTreeSet, VecDeque};

use be_block::{Landed, LiveEdit, Merge, Settling};
use be_commit::{CommitId, MergeResult};
use be_protocol::{ClientId, SessionState};
use be_session::{
    Follower, Inherited, KeptEdits, OpId, Resume, Sequencer, SessionMessage, SessionOp,
};
use block_type_api::{
    Change, Command, EditorId, Endpoint, HeadEvent, Merging, Recipients, ReplicaId, Reply,
    RequestId, SessionTag, Status, Token, Written,
};
use uuid::Uuid;

use crate::{
    FromFileFn, Recorder,
    editor::{EditorOperation, FromEditor, Gesture, PeerMessage, PeerPresence, Snapshot, ToEditor},
    wire::Wire,
};

pub const SEAL_INTERVAL: u64 = 750;

pub const HEARTBEAT_INTERVAL: u64 = be_session::LEASE_MILLISECONDS as u64 / 2;

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
        session: SessionState,
        resynchronize: bool,
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
    Release,
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
    role: Role,
    confirmed: C,
    visible: C,
    unsent: VecDeque<(C::Op, usize, u64)>,
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
    out: Vec<Command>,
}

impl<C, H> AnyReplica for Replica<C, H>
where
    C: LiveEdit + Merge + Clone + Default,
    H: Recorder<C>,
{
    fn start(&mut self, out: &mut Vec<Command>) {
        match self.base {
            Some(head) => self.read(vec![Some(head)], Then::Load),
            None => self.loaded(C::default()),
        }
        self.run(out);
    }

    fn input(&mut self, now: u64, input: Input, out: &mut Vec<Command>) {
        self.now = self.now.max(now);
        match input {
            Input::Read { token, result } => self.answer_read(token, result),
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
        if self.beat_at().is_some_and(|at| self.now >= at) {
            self.heartbeat();
        }
        self.run(out);
    }

    fn wake_at(&self) -> Option<u64> {
        let seal = match self.sealing {
            true => None,
            false => self.owes_since.map(|since| since + SEAL_INTERVAL),
        };
        seal.into_iter().chain(self.beat_at()).min()
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
            out: Vec::new(),
        }
    }

    fn run(&mut self, out: &mut Vec<Command>) {
        loop {
            self.flush_editors();
            if self.waiting.is_some() || self.released {
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
        self.out.push(Command::ReportClean {
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
            Input::Head(_) | Input::Read { .. } | Input::Resumed { .. } => {}
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
            Input::Content { id } => {
                let bytes = self.visible.encode();
                self.reply(id, Reply::Content(bytes));
            }
            Input::Seal { id, force } => self.jobs.push_front(Job::Seal {
                sealing: match force {
                    true => Sealing::Rekey,
                    false => Sealing::Due,
                },
                reply: Some(id),
            }),
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
                if self.confirmed.adopt_session_state(&state).is_err() {
                    return self.unreadable_message();
                }
                self.sealed_state = state;
                if let Role::Follower(follower) = &mut self.role {
                    follower.set_applied(sealed);
                }
                self.rebuild_visible();
                self.snapshot_ops(sealed, sequence, ops, applied);
                self.flush();
            }
            Then::SealedReload {
                head,
                sequence,
                state,
            } => {
                self.confirmed = next();
                if self.confirmed.adopt_session_state(&state).is_err() {
                    self.unreadable_message();
                }
                self.sealed_state = state;
                if let Role::Follower(follower) = &mut self.role {
                    follower.set_applied(sequence);
                }
                self.rebuild_visible();
                self.base = Some(head);
                self.sealed = sequence;
                self.flush();
            }
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
                        reply.map(|id| (id, Reply::Sealed))
                    }
                    Some(Wait::Publish {
                        then: AfterPublish::Replace { reply, .. },
                        ..
                    })
                    | Some(Wait::Stash {
                        then: AfterStash::Replace { reply, .. },
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

    fn published(&mut self, then: AfterPublish<C>, head: Option<CommitId>) {
        match then {
            AfterPublish::Seal { reply, .. } => {
                if let Some(head) = head {
                    self.settle_at(head);
                }
                self.sealing = false;
                if let Some(id) = reply {
                    self.reply(id, Reply::Sealed);
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
                    self.reply(id, Reply::Changed(true));
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
                    self.reply(id, Reply::Sealed);
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
                        self.reply(id, Reply::Changed(false));
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
        self.visible = self.confirmed.clone();
        self.sealed_state = self.confirmed.session_state();
        self.projected = Settling::new(self.visible.clone(), Vec::new());
        self.role = match self.state.is_owner(self.client) {
            true => Role::Owner(Sequencer::new(self.base)),
            false => Role::Follower(Follower::new(self.client)),
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

    fn is_clean(&self) -> bool {
        match &self.role {
            Role::Owner(sequencer) => sequencer.is_clean(),
            Role::Follower(follower) => follower.pending() == 0 && self.unsent.is_empty(),
        }
    }

    fn has_unsealed_work(&self) -> bool {
        matches!(&self.role, Role::Owner(sequencer) if !sequencer.is_clean())
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
        match &mut self.role {
            Role::Owner(sequencer) => {
                let id = OpId {
                    client: self.client,
                    counter: sequencer.sequence() + 1,
                };
                let Some(op) = sequencer.accept(id, payload) else {
                    return true;
                };
                self.confirmed.apply(&operation);
                self.broadcast(&SessionMessage::Accepted { op });
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

    fn report(&mut self) {
        let status = Status {
            head: self.base,
            owner: self.is_owner(),
            clean: self.is_clean(),
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
        self.out.push(Command::Send {
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
        let since = follower.applied();
        let owner = self.state.owner;
        self.send_wire(owner, &Wire::Tag(self.tag));
        self.send_wire(owner, &Wire::Session(SessionMessage::Catchup { since }));
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
                self.broadcast(&SessionMessage::Accepted { op });
            }
            SessionMessage::Accepted { op } => match self.role {
                Role::Owner(_) => self.inherit_accepted(&op),
                Role::Follower(_) if !self.catching_up => self.apply_accepted(&op),
                Role::Follower(_) => {}
            },
            SessionMessage::Catchup { since } => {
                let Role::Owner(sequencer) = &self.role else {
                    return;
                };
                if sequencer.inherited() > 0 {
                    self.reload = true;
                }
                let message = SessionMessage::Snapshot {
                    head: sequencer.head(),
                    sealed: self.sealed,
                    sequence: sequencer.sequence(),
                    ops: sequencer.since(since.min(self.sealed)),
                    state: self.sealed_state.clone(),
                };
                self.send(Some(from), &message);
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
                if head != self.base || applied < sealed {
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
                            },
                        );
                    }
                    None => self.sealed_state = self.confirmed.session_state(),
                }
                self.base = Some(head);
                self.sealed = sequence;
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
        let mut sequencer =
            Sequencer::continuing(self.base, applied, applied.saturating_sub(self.sealed));
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
        self.role = Role::Owner(sequencer);
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
                    self.broadcast(&SessionMessage::Accepted { op });
                }
            }
        }
    }

    fn claim(&mut self) {
        if !self.loaded || self.catching_up || self.is_owner() {
            return;
        }
        self.claimed = Some(self.state.generation);
        self.out.push(Command::ClaimLease {
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
        self.out.push(Command::ClaimLease {
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
            Role::Owner(sequencer) => !sequencer.is_clean(),
            Role::Follower(follower) => !self.is_clean() || follower.applied() > self.sealed,
        }
    }

    fn reconcile(&mut self) {
        let token = self.token();
        self.out.push(Command::Resume {
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
                self.reply(id, Reply::Sealed);
            }
            return;
        }
        if self.diverged {
            self.sealing = false;
            self.error(DIVERGED);
            if let Some(id) = reply {
                self.reply(id, Reply::Sealed);
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
                self.reply(id, Reply::Changed(false));
            }
            return;
        }
        if outright && self.has_unsaved() {
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
        let reload = std::mem::take(&mut self.reload).then(|| self.sealed_state.clone());
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
        });
        self.beaten = Some(self.now);
        self.active = false;
        self.out.push(Command::ReportClean {
            replica: self.id,
            generation: self.state.generation,
            clean_at: Some(head),
        });
    }
}
