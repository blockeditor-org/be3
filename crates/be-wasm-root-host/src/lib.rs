//! The host's half of the root guest's entry (plans/host_as_platform_adapter.md, The root
//! guest): `Root` runs be-host's own module inline on the host's frame thread, and serves it the
//! sandbox (be-wasm-sandbox-host) and `be3:root`'s `host` (crates/be-wasm-root-api).
//!
//! Input is the root's `input.deliver`, an op the host starts and steps the root through with the
//! frame being drawn, or else the next one, so the root routes it by the layers it drew last.
//!
//! A frame is the root's `frames.frame`, an op the root serves, which the host starts and steps
//! the root through until it answers. A step that leaves the frame unanswered is the root's
//! `Pending`: what it waits on is the ops it left open, its children's frames (`instance.next`)
//! and `deadline`. The host steps the root again once one of them has something for it, and
//! answers `deadline` once the frame's deadline passes. So the root writes ordinary async code
//! that awaits its children, and answers within one host frame without ever blocking its own
//! thread. `Core` is that stepping, the same on every platform; how the host waits between steps
//! is not:
//!
//! - natively (`native`), `Root::frame` waits on a bell its instances, compiles and own wakes
//!   ring, up to the deadline, so it returns the drawn frame;
//! - in the browser (`web`), the page may never wait (decisions/the_web_page_never_waits.md), so
//!   `Root::frame`, called from `requestAnimationFrame`, steps the root for a slice of the page's
//!   thread at most and returns; a task resumes it whenever its workers answer, a compile
//!   finishes, it wakes itself, its slice ran out or the deadline passes, and the frame is handed
//!   over once the root answers.

use std::time::{Duration, Instant};

use be_wasm_guest_host::STEP_TIMEOUT;
use be_wasm_ops::{Connection, Incoming, OpError, OpId, Outcome, Reply, Side, Ticket};
use be_wasm_root_api::{
    FrameDone, FrameRequest, embed, host::CoreView, host::HostStatistics, host::LockState, host::Notice,
    host::PowerAction, host::ShownNotices, host::StatisticRow,
};
use be_wasm_sandbox_host::Server;
pub use be_wasm_sandbox_host::{Loader, Staged};

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::Root;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::Root;

/// The root guest as its host steps it, inline on the host's thread.
trait Inline {
    /// Queues `frames` for the root's next step.
    fn deliver(&mut self, frames: Vec<Vec<u8>>);
    /// Steps the root once, or says why it trapped or panicked.
    fn step(&mut self) -> Result<(), String>;
    /// The frames the root sent.
    fn take_outbound(&mut self) -> Vec<Vec<u8>>;
    /// Whether the root woke itself since the last call.
    fn take_wake(&mut self) -> bool;
}

/// Where a frame got to.
enum Progress {
    /// The root answered the frame, or why it never will: it trapped, panicked or took longer
    /// than its patience.
    Drawn(Result<FrameDone, String>),
    /// The root waits for what its host has not had yet: step it again once something it waits
    /// on rings, or at `until` at the latest.
    Pending { until: Instant },
    /// The root used its slice of the thread while it still had work: step it again soon, after
    /// whatever else the thread has to do.
    Yielded,
}

/// The frame being drawn.
struct Drawing {
    ticket: Ticket<FrameDone>,
    deadline: Instant,
    /// When the root itself counts as stopped: its patience past the deadline.
    give_up: Instant,
    /// Whether the deadline passed.
    passed: bool,
}

/// The answer to the root's `host.paste`: a clip item in be-wasm-ops' encoding, or none.
type PasteReply = Reply<Option<Vec<u8>>, be_wasm_ops::wire::Optional<be_wasm_ops::wire::Bytes>>;

/// The stepping both platforms share: the root, its connection, its sandbox and the frame it
/// draws.
struct Core<G> {
    guest: G,
    connection: Connection,
    sandbox: Server,
    /// The `deadline` calls of the frame being drawn, answered once its deadline passes.
    deadlines: Vec<Reply<()>>,
    drawing: Option<Drawing>,
    /// How the frame being drawn ended, once it did.
    drawn: Option<Outcome>,
    /// Why the root stopped, once it did: it never runs again.
    failure: Option<String>,
    /// How long past a frame's deadline the root itself may take before it counts as stopped.
    patience: Duration,
    /// The fonts the host bundles, which `host.fonts` answers: be-sdk-api's `Fonts`, encoded.
    fonts: Vec<u8>,
    /// The paintings asked of the root and not answered yet.
    painting: Option<Ticket<Vec<u8>, be_wasm_ops::wire::Bytes>>,
    /// What the root answered it painted, until the host takes it.
    painted: Option<Vec<u8>>,
    /// The host's notices, which `host.notices` answers: the generation counts each change.
    notices: ShownNotices,
    /// The root's `host.notices` calls waiting for the notices to change from those it saw.
    watching: Vec<Reply<ShownNotices>>,
    /// How the root answered notices (`host.answer-notice`), until the host takes them.
    answers: Vec<(u64, Option<String>)>,
    /// What the root copied (`host.copy`), each copy's items, until the host takes them.
    copies: Vec<Vec<Vec<u8>>>,
    /// The pastes the root asked for (`host.paste`), by id with the formats each takes, until
    /// the host takes them to read its clipboard.
    pastes: Vec<(u64, Vec<String>)>,
    /// The root's `host.paste` calls waiting for the host to answer them, by id.
    pasting: Vec<(u64, PasteReply)>,
    /// The id the next paste asked for is given.
    next_paste: u64,
    /// Whether the host delivers the paste shortcut as a paste itself (`host.paste-shortcut`).
    paste_shortcut: bool,
    /// Whether the root watches the host's statistics (`host.watch-statistics`).
    watching_statistics: bool,
    /// The host's statistics, which `host.statistics` answers: the generation counts each change.
    statistics: HostStatistics,
    /// The root's `host.statistics` calls waiting for them to change from those it saw.
    statistics_watchers: Vec<Reply<HostStatistics>>,
    /// The session's screen lock, which `host.lock` answers: the generation counts each change.
    lock: LockState,
    /// The root's `host.lock` calls waiting for it to change from what they saw.
    lock_watchers: Vec<Reply<LockState>>,
    /// Whether the root asked to lock the session (`host.lock-session`) since the host last took
    /// it.
    lock_asked: bool,
    /// The passwords the root handed over (`host.unlock`), by id, until the host takes them to
    /// check.
    unlocks: Vec<(u64, Password)>,
    /// The root's `host.unlock` calls waiting for the host's answer, by id.
    unlocking: Vec<(u64, Reply<bool>)>,
    /// The id the next password handed over is given.
    next_unlock: u64,
    /// What the root chose on its lock screen besides unlocking (`host.lock-power`), until the
    /// host takes it.
    lock_powers: Vec<PowerAction>,
    /// What be-host's core shows, which `host.core` answers: the generation counts each change.
    core: CoreView,
    /// The root's `host.core` calls waiting for it to change from what they saw.
    core_watchers: Vec<Reply<CoreView>>,
}

/// A password the root handed over to check (`host.unlock`): its bytes as typed, zeroed when it
/// is dropped, and never printed.
pub struct Password(Vec<u8>);

impl Password {
    /// The bytes as typed, which the host checks and drops.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

// unsafe(memory)
impl Drop for Password {
    fn drop(&mut self) {
        for byte in self.0.iter_mut() {
            // Volatile, so the zeroing is not optimised away as a dead store.
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Password(..)")
    }
}

impl<G: Inline> Core<G> {
    /// Takes over `guest`, the root, which has started, and what it sent while it started.
    fn new(guest: G, sandbox: Server) -> Result<Self, String> {
        let mut core = Self {
            guest,
            connection: Connection::new(Side::Host),
            sandbox,
            deadlines: Vec::new(),
            drawing: None,
            drawn: None,
            failure: None,
            patience: STEP_TIMEOUT,
            fonts: Vec::new(),
            painting: None,
            painted: None,
            notices: ShownNotices {
                generation: 1,
                shown: Vec::new(),
            },
            watching: Vec::new(),
            answers: Vec::new(),
            copies: Vec::new(),
            pastes: Vec::new(),
            pasting: Vec::new(),
            next_paste: 0,
            paste_shortcut: false,
            watching_statistics: false,
            statistics: HostStatistics {
                generation: 1,
                performance: Vec::new(),
                memory: Vec::new(),
                memory_file: None,
            },
            statistics_watchers: Vec::new(),
            lock: LockState {
                generation: 1,
                available: false,
                locked: false,
                checking: false,
                refused: None,
                user: String::new(),
                power: Vec::new(),
            },
            lock_watchers: Vec::new(),
            lock_asked: false,
            unlocks: Vec::new(),
            unlocking: Vec::new(),
            next_unlock: 0,
            lock_powers: Vec::new(),
            core: CoreView {
                generation: 1,
                saving: be_wasm_root_api::Saving {
                    unsaved: false,
                    saves: 0,
                    unsent: 0,
                    sync: be_wasm_root_api::SyncState::Saved,
                },
            },
            core_watchers: Vec::new(),
        };
        core.received()?;
        Ok(core)
    }

    /// Whether a frame is being drawn, which on the web may wait between calls.
    #[cfg(target_arch = "wasm32")]
    fn drawing(&self) -> bool {
        self.drawing.is_some()
    }

    /// Hands the root `window`'s input, which it takes in with the steps of the frame being
    /// drawn, or else of the next one, ahead of drawing it, so the root routes it by the layers
    /// it drew last. Its answer, which the root gives once it has routed, is not waited for. A root that stopped hears nothing, and says why.
    fn input(&mut self, window: Option<u64>, events: Vec<u8>) -> Result<(), String> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let _answered = embed::exports::input::deliver(&mut self.connection, window, events);
        Ok(())
    }

    /// Asks the root what its own surfaces painted, which it answers as it steps through the
    /// next frame (`take_paintings`). A root that stopped says why.
    fn ask_paintings(&mut self) -> Result<(), String> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        if self.painting.is_none() {
            self.painting = Some(embed::exports::paintings::paintings(&mut self.connection));
        }
        Ok(())
    }

    /// What the root answered `ask_paintings` with, once it did.
    fn take_paintings(&mut self) -> Option<Vec<u8>> {
        self.painted.take()
    }

    /// Shows the person `shown`, the host's notices, oldest first, in place of those shown
    /// before: the root's `host.notices` calls waiting for a change answer, which its next
    /// step hears. Answers whether any did, so the host steps the root for them.
    fn show_notices(&mut self, shown: Vec<Notice>) -> bool {
        if shown == self.notices.shown {
            return false;
        }
        self.notices = ShownNotices {
            generation: self.notices.generation + 1,
            shown,
        };
        let watching = std::mem::take(&mut self.watching);
        let answered = !watching.is_empty();
        for reply in watching {
            reply.send(&mut self.connection, &self.notices);
        }
        answered
    }

    /// How the root answered the host's notices since the last call, in order: each notice's
    /// id and the key of the action the person chose, or none where they dismissed it.
    fn take_answers(&mut self) -> Vec<(u64, Option<String>)> {
        std::mem::take(&mut self.answers)
    }

    /// What the root copied since the last call (`host.copy`), oldest first: each copy's items,
    /// richest first, each be-lib-ui-types' `ClipItem` in be-wasm-ops' encoding. The host puts
    /// each on its clipboard in turn, so the last stays.
    fn take_copies(&mut self) -> Vec<Vec<Vec<u8>>> {
        std::mem::take(&mut self.copies)
    }

    /// The pastes the root asked for since the last call (`host.paste`), oldest first: each its
    /// id, which `answer_paste` names, and the formats it takes, preferred first.
    fn take_pastes(&mut self) -> Vec<(u64, Vec<String>)> {
        std::mem::take(&mut self.pastes)
    }

    /// Answers the paste `id` with what the host's clipboard held of the formats it takes: a
    /// `ClipItem` in be-wasm-ops' encoding, or none. Answers whether the root waited for it, so
    /// the host steps the root for it; a paste the root gave up on, or answered already, changes
    /// nothing.
    fn answer_paste(&mut self, id: u64, item: Option<Vec<u8>>) -> bool {
        let Some(at) = self.pasting.iter().position(|(asked, _)| *asked == id) else {
            return false;
        };
        let (_, reply) = self.pasting.remove(at);
        reply.send(&mut self.connection, &item);
        true
    }

    /// Shows the root the host's statistics, in place of those shown before: its
    /// `host.statistics` calls waiting for a change answer. Answers whether any did, so the host
    /// steps the root for them.
    fn show_statistics(
        &mut self,
        performance: Vec<StatisticRow>,
        memory: Vec<StatisticRow>,
        memory_file: Option<String>,
    ) -> bool {
        let shown = &self.statistics;
        if shown.performance == performance
            && shown.memory == memory
            && shown.memory_file == memory_file
        {
            return false;
        }
        self.statistics = HostStatistics {
            generation: self.statistics.generation + 1,
            performance,
            memory,
            memory_file,
        };
        let watching = std::mem::take(&mut self.statistics_watchers);
        let answered = !watching.is_empty();
        for reply in watching {
            reply.send(&mut self.connection, &self.statistics);
        }
        answered
    }

    /// Shows the root the session's screen lock (`host.lock`), in place of what it showed:
    /// `shown`'s generation is the host's own counting. Answers whether a root's call waited for
    /// it, so the host steps the root.
    fn show_lock(&mut self, shown: LockState) -> bool {
        let LockState { generation, .. } = self.lock;
        let changed = LockState {
            generation,
            ..shown.clone()
        } != self.lock;
        if !changed {
            return false;
        }
        self.lock = LockState {
            generation: generation + 1,
            ..shown
        };
        let watching = std::mem::take(&mut self.lock_watchers);
        let answered = !watching.is_empty();
        for reply in watching {
            reply.send(&mut self.connection, &self.lock);
        }
        answered
    }

    /// Shows the root what be-host's core shows (`host.core`), in place of what it showed:
    /// `shown`'s generation is the host's own counting. Answers whether a root's call waited for
    /// it.
    fn show_core(&mut self, shown: CoreView) -> bool {
        let CoreView { generation, .. } = self.core;
        if (CoreView {
            generation,
            ..shown.clone()
        }) == self.core
        {
            return false;
        }
        self.core = CoreView {
            generation: generation + 1,
            ..shown
        };
        let watching = std::mem::take(&mut self.core_watchers);
        let answered = !watching.is_empty();
        for reply in watching {
            reply.send(&mut self.connection, &self.core);
        }
        answered
    }

    /// Whether the root asked to lock the session since the last call.
    fn take_lock_asked(&mut self) -> bool {
        std::mem::take(&mut self.lock_asked)
    }

    /// The passwords the root handed over since the last call, oldest first, each with the id
    /// `answer_unlock` names.
    fn take_unlocks(&mut self) -> Vec<(u64, Password)> {
        std::mem::take(&mut self.unlocks)
    }

    /// Answers the password `id` handed over: whether it unlocked the session. Answers whether
    /// the root waited for it.
    fn answer_unlock(&mut self, id: u64, unlocked: bool) -> bool {
        let Some(at) = self.unlocking.iter().position(|(asked, _)| *asked == id) else {
            return false;
        };
        let (_, reply) = self.unlocking.remove(at);
        reply.send(&mut self.connection, &unlocked);
        true
    }

    /// What the root chose on its lock screen besides unlocking since the last call.
    fn take_lock_powers(&mut self) -> Vec<PowerAction> {
        std::mem::take(&mut self.lock_powers)
    }

    /// Asks the root for a frame for `request`, drawn by `deadline`, or says why the root drew
    /// nothing since it stopped.
    fn begin(&mut self, request: FrameRequest, deadline: Instant) -> Result<(), String> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let ticket = embed::exports::frames::frame(&mut self.connection, request);
        self.drawing = Some(Drawing {
            ticket,
            deadline,
            give_up: deadline.max(Instant::now()) + self.patience,
            passed: false,
        });
        Ok(())
    }

    /// Steps the root as far as it goes without waiting: until it answers the frame, or waits
    /// for something its host has not had yet, or, given a `slice`, until it stepped that long.
    fn advance(&mut self, slice: Option<Duration>) -> Progress {
        let yield_at = slice.map(|slice| Instant::now() + slice);
        loop {
            let Some(drawing) = &self.drawing else {
                return Progress::Drawn(Err("no frame is being drawn".to_owned()));
            };
            let now = Instant::now();
            // Checked before stepping, so a root that keeps waking itself, or children that keep
            // sending, cannot hold the frame past it.
            if now >= drawing.give_up {
                let failure = format!(
                    "the root's frame took over {}ms past its deadline",
                    self.patience.as_millis()
                );
                return self.finish(Err(failure));
            }
            if now >= drawing.deadline {
                self.pass_deadline();
            }
            self.serve();
            let Some(drawing) = &self.drawing else {
                continue;
            };
            let ticket = drawing.ticket;
            // Past the deadline, the root itself may take until its patience runs out.
            let until = match drawing.passed {
                true => drawing.give_up,
                false => drawing.deadline,
            };
            if let Some(outcome) = self.drawn.take() {
                let drawn = ticket
                    .decode(outcome)
                    .map_err(|error| format!("the root did not draw its frame: {error}"));
                return self.finish(drawn);
            }
            match self.step() {
                Ok(true) if yield_at.is_some_and(|at| Instant::now() >= at) => {
                    return Progress::Yielded;
                }
                Ok(true) => {}
                Ok(false) => return Progress::Pending { until },
                Err(failure) => return self.finish(Err(failure)),
            }
        }
    }

    /// Ends the frame being drawn with `drawn`: every `deadline` call of it answers, and a root
    /// that failed draws nothing again.
    fn finish(&mut self, drawn: Result<FrameDone, String>) -> Progress {
        self.drawing = None;
        self.drawn = None;
        for reply in std::mem::take(&mut self.deadlines) {
            reply.send(&mut self.connection, &());
        }
        if let Err(failure) = &drawn {
            self.failure = Some(failure.clone());
        }
        Progress::Drawn(drawn)
    }

    /// The frame's deadline passed: the root hears it, and every later `deadline` call of the
    /// frame answers at once.
    fn pass_deadline(&mut self) {
        if let Some(drawing) = &mut self.drawing {
            drawing.passed = true;
        }
        for reply in std::mem::take(&mut self.deadlines) {
            reply.send(&mut self.connection, &());
        }
    }

    /// Steps the root with what the host has for it, if anything, and takes what it sent.
    /// Answers whether it stepped.
    fn step(&mut self) -> Result<bool, String> {
        let frames = self.connection.take_outbound();
        let woken = self.guest.take_wake();
        if frames.is_empty() && !woken {
            return Ok(false);
        }
        self.guest.deliver(frames);
        self.guest.step()?;
        self.received()?;
        Ok(true)
    }

    /// Takes the frames the root sent.
    fn received(&mut self) -> Result<(), String> {
        for frame in self.guest.take_outbound() {
            self.connection
                .receive(&frame)
                .map_err(|error| format!("the root sent a frame the host cannot read: {error}"))?;
        }
        Ok(())
    }

    /// Answers the root's calls, and whatever its instances and compiles have for it.
    fn serve(&mut self) {
        while let Some(event) = self.connection.next_event() {
            match event {
                Incoming::Started(started) => {
                    if be_wasm_sandbox_api::embed::sandbox::Call::accepts(&started) {
                        if let Some(call) = be_wasm_sandbox_api::embed::sandbox::Call::accept(
                            &mut self.connection,
                            started,
                        ) {
                            self.sandbox.call(call, &mut self.connection);
                        }
                    } else if let Some(call) = embed::Call::accept(&mut self.connection, started) {
                        self.call(call);
                    }
                }
                Incoming::Canceled { op } => {
                    if self.sandbox.canceled(op, &mut self.connection) {
                        continue;
                    }
                    if let Some(at) = self.deadlines.iter().position(|reply| reply.op() == op) {
                        self.deadlines
                            .remove(at)
                            .fail(&mut self.connection, OpError::Canceled);
                    } else if let Some(at) = self.watching.iter().position(|reply| reply.op() == op)
                    {
                        self.watching
                            .remove(at)
                            .fail(&mut self.connection, OpError::Canceled);
                    } else if let Some(at) =
                        self.core_watchers.iter().position(|reply| reply.op() == op)
                    {
                        self.core_watchers
                            .remove(at)
                            .fail(&mut self.connection, OpError::Canceled);
                    } else if let Some(at) =
                        self.lock_watchers.iter().position(|reply| reply.op() == op)
                    {
                        self.lock_watchers
                            .remove(at)
                            .fail(&mut self.connection, OpError::Canceled);
                    } else if let Some(at) =
                        self.unlocking.iter().position(|(_, reply)| reply.op() == op)
                    {
                        let (id, reply) = self.unlocking.remove(at);
                        // Unchecked, its password goes with it.
                        self.unlocks.retain(|(asked, _)| *asked != id);
                        reply.fail(&mut self.connection, OpError::Canceled);
                    } else if let Some(at) = self
                        .statistics_watchers
                        .iter()
                        .position(|reply| reply.op() == op)
                    {
                        self.statistics_watchers
                            .remove(at)
                            .fail(&mut self.connection, OpError::Canceled);
                    } else if let Some(at) =
                        self.pasting.iter().position(|(_, reply)| reply.op() == op)
                    {
                        let (id, reply) = self.pasting.remove(at);
                        self.pastes.retain(|(asked, _)| *asked != id);
                        reply.fail(&mut self.connection, OpError::Canceled);
                    }
                }
                Incoming::Completed { op, outcome } if self.drawing_op() == Some(op) => {
                    self.drawn = Some(outcome);
                }
                Incoming::Completed { op, outcome }
                    if self.painting.is_some_and(|ticket| ticket.op() == op) =>
                {
                    if let Some(ticket) = self.painting.take() {
                        // A root that cannot answer paints nothing the host can show.
                        self.painted = Some(ticket.decode(outcome).unwrap_or_default());
                    }
                }
                // Includes a frame answered after the host gave up on it: no one waits for it.
                Incoming::Completed { .. }
                | Incoming::Yielded { .. }
                | Incoming::Credited { .. } => {}
            }
        }
        self.sandbox.poll(&mut self.connection);
    }

    fn drawing_op(&self) -> Option<OpId> {
        self.drawing.as_ref().map(|drawing| drawing.ticket.op())
    }

    fn call(&mut self, call: embed::Call) {
        match call {
            embed::Call::Host(embed::host::Call::Deadline { reply }) => match &self.drawing {
                Some(drawing) if !drawing.passed => self.deadlines.push(reply),
                Some(_) | None => reply.send(&mut self.connection, &()),
            },
            embed::Call::Host(embed::host::Call::Fonts { reply }) => {
                reply.send(&mut self.connection, &self.fonts);
            }
            embed::Call::Host(embed::host::Call::Notices { seen, reply }) => {
                match seen == self.notices.generation {
                    true => self.watching.push(reply),
                    false => reply.send(&mut self.connection, &self.notices),
                }
            }
            embed::Call::Host(embed::host::Call::AnswerNotice { id, action, reply }) => {
                self.answers.push((id, action));
                reply.send(&mut self.connection, &());
            }
            embed::Call::Host(embed::host::Call::Copy { items, reply }) => {
                self.copies.push(items);
                reply.send(&mut self.connection, &());
            }
            embed::Call::Host(embed::host::Call::WatchStatistics { watching, reply }) => {
                self.watching_statistics = watching;
                reply.send(&mut self.connection, &());
                // No change comes while nothing watches: a call still waiting for one hears
                // the statistics as they are, so the root's task waiting on it ends.
                if !watching {
                    for reply in std::mem::take(&mut self.statistics_watchers) {
                        reply.send(&mut self.connection, &self.statistics);
                    }
                }
            }
            embed::Call::Host(embed::host::Call::Statistics { seen, reply }) => {
                match seen == self.statistics.generation {
                    true => self.statistics_watchers.push(reply),
                    false => reply.send(&mut self.connection, &self.statistics),
                }
            }
            embed::Call::Host(embed::host::Call::Lock { seen, reply }) => {
                match seen == self.lock.generation {
                    true => self.lock_watchers.push(reply),
                    false => reply.send(&mut self.connection, &self.lock),
                }
            }
            embed::Call::Host(embed::host::Call::Core { seen, reply }) => {
                match seen == self.core.generation {
                    true => self.core_watchers.push(reply),
                    false => reply.send(&mut self.connection, &self.core),
                }
            }
            embed::Call::Host(embed::host::Call::LockSession { reply }) => {
                self.lock_asked = true;
                reply.send(&mut self.connection, &());
            }
            embed::Call::Host(embed::host::Call::Unlock { password, reply }) => {
                let password = Password(password);
                // Only a locked session is unlocked: anything else is a no, and its password
                // is dropped unchecked.
                if !self.lock.locked {
                    reply.send(&mut self.connection, &false);
                    return;
                }
                let id = self.next_unlock;
                self.next_unlock += 1;
                self.unlocks.push((id, password));
                self.unlocking.push((id, reply));
            }
            embed::Call::Host(embed::host::Call::LockPower { action, reply }) => {
                self.lock_powers.push(action);
                reply.send(&mut self.connection, &());
            }
            embed::Call::Host(embed::host::Call::PasteShortcut { reply }) => {
                reply.send(&mut self.connection, &self.paste_shortcut);
            }
            embed::Call::Host(embed::host::Call::Paste { formats, reply }) => {
                let id = self.next_paste;
                self.next_paste += 1;
                self.pastes.push((id, formats));
                self.pasting.push((id, reply));
            }
        }
    }
}

#[cfg(test)]
mod tests;
