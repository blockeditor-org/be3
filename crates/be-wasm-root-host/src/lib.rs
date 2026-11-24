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
pub use be_wasm_password::Password;
use be_wasm_root_api::{
    FrameDone, FrameRequest, embed, host::BlockCatalog, host::HostStatistics, host::LockState,
    host::Notice, host::PowerAction, host::ShownNotices, host::StatisticRow,
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
    /// The host's catalog, which `host.block-types` answers: the generation counts each change.
    block_types: BlockCatalog,
    /// The root's `host.block-types` calls waiting for the catalog to change from the one it saw.
    block_types_watchers: Vec<Reply<BlockCatalog>>,
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
    /// The host's memory over time, which `host.memory-history` answers.
    memory_history: be_wasm_root_api::MemoryMinutes,
    /// The root's `host.memory-history` calls waiting for it to change from what they saw.
    memory_watchers: Vec<Reply<be_wasm_root_api::MemoryMinutes>>,
    /// The session's screen lock, which `host.lock` answers: the generation counts each change.
    lock: LockState,
    /// The root's `host.lock` calls waiting for it to change from what they saw.
    lock_watchers: Vec<Reply<LockState>>,
    /// Whether the root asked to lock the session (`host.lock-session`) since the host last took
    /// it.
    lock_asked: bool,
    /// The password the root handed over (`host.unlock`), until the host takes it to check.
    unlock: Option<Password>,
    /// Whether the host took a password to check and has not shown the lock since: until it
    /// does, it says nothing of `checking`, so another is dropped meanwhile.
    taken: bool,
    /// What the root chose on its lock screen besides unlocking (`host.lock-power`), until the
    /// host takes it.
    lock_powers: Vec<PowerAction>,
    /// Whether the person's changes are saved, which `host.saving` answers: the generation
    /// counts each change.
    saving: be_wasm_root_api::SavingState,
    /// The root's `host.saving` calls waiting for it to change from what they saw.
    saving_watchers: Vec<Reply<be_wasm_root_api::SavingState>>,
    /// Whether the host asks to keep display settings, which `host.displays` answers.
    displays: be_wasm_root_api::DisplayState,
    /// The root's `host.displays` calls waiting for it to change from what they saw.
    display_watchers: Vec<Reply<be_wasm_root_api::DisplayState>>,
    /// How the root answered the display question asked (`host.answer-display`), each round
    /// and whether kept, until the host takes them: only answers to the round asked when the
    /// answer came.
    display_answers: Vec<(u64, bool)>,
    /// What the block stack shows the person, which `host.core` answers.
    core: be_wasm_root_api::CoreView,
    /// The root's `host.core` calls waiting for it to change from what they saw.
    core_watchers: Vec<Reply<be_wasm_root_api::CoreView>>,
    /// What the root asked of the block stack (`host.ask-core`), until the host takes it.
    core_commands: Vec<be_wasm_root_api::CoreCommand>,
    /// What the root asked to make a secret (`host.make-secret`), by the id the host answers it
    /// by, until the host takes it.
    secret_requests: Vec<(u64, be_wasm_root_api::SecretCommand)>,
    /// The root's `host.make-secret` calls, by that id, until the host answers them
    /// (`answer_secret`).
    secret_replies: Vec<(u64, Reply<Option<be_wasm_root_api::CoreSecret>>)>,
    /// The id the next secret the root asks for is handed to the host with.
    next_secret: u64,
    /// The blocks the root asked the editor of (`host.editor-of`), by the id the host answers
    /// by, until the host takes them.
    editor_asks: Vec<(u64, be_wasm_root_api::ShownBlock)>,
    /// The editors the root asked the host to open (`host.open-editor`), until it takes them.
    openings: Vec<EditorOpening>,
    /// The root's `host.editor-of` and `host.open-editor` calls, by that id, until the host
    /// answers them.
    editor_replies: Vec<(u64, EditorReply)>,
    /// The id the next of those is handed to the host with.
    next_editor: u64,
    /// What the root forwarded the host of the apps it spawned (`host.forward`), each app's
    /// frames in order, until the host takes them.
    forwarded: Vec<(u64, Vec<Vec<u8>>)>,
    /// What the host sent the apps the root spawned, until the root takes it
    /// (`host.forwarded`).
    relaying: Vec<be_wasm_root_api::RelayedFrames>,
    /// Whether its sandbox answered the root (as it polled, or in a call it took), or frames
    /// were relayed to it, since it was last stepped: what it has yet to take in.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    unstepped: bool,
    /// The root's `host.forwarded` call, waiting for the host to send its apps something.
    relay_watcher: Option<Reply<Vec<be_wasm_root_api::RelayedFrames>>>,
    /// The children the root shows of the editors the host opened (`host.show-children`), the
    /// latest of each editor's view, until the host takes them.
    shown_children: Vec<ShownChildren>,
    /// What the root told the host's editors of their children (`host.tell-placer`), by the
    /// root's instance and the editor, until the host takes it.
    placer_events: Vec<(u64, u64, be_wasm_root_api::PlacerEvent)>,
    /// Where the host's editors the root shows as placed children show their content
    /// (`host.place-content`), until the host takes them.
    placed_contents: Vec<be_wasm_root_api::PlacedContent>,
    /// The dock updates the root made for the host's editors (`host.dock`), by the root's
    /// instance and the editor, each as the app protocol encodes them, until the host takes them.
    dock_updates: Vec<(u64, u64, Vec<u8>)>,
}

/// The children an editor the host opened placed in one of its views, as the root shows them
/// (`host.show-children`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownChildren {
    /// The op of the sandbox `instance` resource the editor's app runs in.
    pub instance: u64,
    /// The editor, as `open-editor` named it.
    pub editor: u64,
    /// Its view, as the app protocol's `view-id`.
    pub view: u64,
    pub children: Vec<be_wasm_root_api::ShownChild>,
}

/// An editor the root asked its host to open (`host.open-editor`), which the host answers by
/// `id` (`Root::answer_opening`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorOpening {
    pub id: u64,
    /// The op of the sandbox `instance` resource the app runs in.
    pub instance: u64,
    /// The staged app it runs, by its manifest's id, while it runs one: what the host opens the
    /// editor in, whatever app the root meant.
    pub app: Option<String>,
    /// The block, and the editor and view block its placement names.
    pub shown: be_wasm_root_api::ShownBlock,
}

/// A root's call about the editors its host opens, waiting for the host's answer.
enum EditorReply {
    Of(Reply<Result<be_wasm_root_api::EditorApp, String>>),
    Open(Reply<Result<be_wasm_root_api::OpenedEditor, String>>),
}

impl EditorReply {
    fn op(&self) -> OpId {
        match self {
            Self::Of(reply) => reply.op(),
            Self::Open(reply) => reply.op(),
        }
    }

    fn fail(self, connection: &mut Connection, error: OpError) {
        match self {
            Self::Of(reply) => reply.fail(connection, error),
            Self::Open(reply) => reply.fail(connection, error),
        }
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
            block_types: BlockCatalog {
                generation: 1,
                catalog: Vec::new(),
                debug_templates: Vec::new(),
            },
            block_types_watchers: Vec::new(),
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
            memory_history: be_wasm_root_api::MemoryMinutes {
                generation: 1,
                ..be_wasm_root_api::MemoryMinutes::default()
            },
            memory_watchers: Vec::new(),
            lock: LockState {
                generation: 1,
                available: false,
                locked: false,
                checking: false,
                refused: None,
                user: String::new(),
                power: Vec::new(),
                retry_at: None,
            },
            lock_watchers: Vec::new(),
            lock_asked: false,
            unlock: None,
            taken: false,
            lock_powers: Vec::new(),
            saving: be_wasm_root_api::SavingState {
                generation: 1,
                unsaved: false,
                saves: 0,
                unsent: 0,
                sync: be_wasm_root_api::SyncState::Saved,
            },
            saving_watchers: Vec::new(),
            displays: be_wasm_root_api::DisplayState {
                generation: 1,
                asking: None,
            },
            display_watchers: Vec::new(),
            display_answers: Vec::new(),
            core: be_wasm_root_api::CoreView {
                generation: 1,
                ..be_wasm_root_api::CoreView::default()
            },
            core_watchers: Vec::new(),
            core_commands: Vec::new(),
            secret_requests: Vec::new(),
            secret_replies: Vec::new(),
            next_secret: 1,
            editor_asks: Vec::new(),
            openings: Vec::new(),
            editor_replies: Vec::new(),
            next_editor: 1,
            forwarded: Vec::new(),
            relaying: Vec::new(),
            unstepped: false,
            relay_watcher: None,
            shown_children: Vec::new(),
            placer_events: Vec::new(),
            dock_updates: Vec::new(),
            placed_contents: Vec::new(),
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

    /// Hands the root the host's catalog, `catalog` and `debug_templates` as `host.block-types`
    /// carries them, in place of the one handed before: the root's calls waiting for a change
    /// answer, which its next step hears. Answers whether any did, so the host steps the root
    /// for them.
    fn show_block_types(&mut self, catalog: Vec<u8>, debug_templates: Vec<u8>) -> bool {
        if (&catalog, &debug_templates)
            == (&self.block_types.catalog, &self.block_types.debug_templates)
        {
            return false;
        }
        self.block_types = BlockCatalog {
            generation: self.block_types.generation + 1,
            catalog,
            debug_templates,
        };
        let watching = std::mem::take(&mut self.block_types_watchers);
        let answered = !watching.is_empty();
        for reply in watching {
            reply.send(&mut self.connection, &self.block_types);
        }
        answered
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

    /// Shows the root the host's memory over time (`host.memory-history`), in place of what it
    /// showed. Answers whether a root's call waited for it, so the host steps the root.
    fn show_memory_history(
        &mut self,
        points: Vec<be_wasm_root_api::MemoryPoint>,
        now: u64,
    ) -> bool {
        if self.memory_history.points == points && self.memory_history.now == now {
            return false;
        }
        self.memory_history = be_wasm_root_api::MemoryMinutes {
            generation: self.memory_history.generation + 1,
            points,
            now,
        };
        let watching = std::mem::take(&mut self.memory_watchers);
        let answered = !watching.is_empty();
        for reply in watching {
            reply.send(&mut self.connection, &self.memory_history);
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
        // The host has said how the password it took stands.
        self.taken = false;
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

    /// Shows the root whether the person's changes are saved (`host.saving`), in place of what
    /// it showed: `shown`'s generation is the host's own counting. Answers whether a root's call
    /// waited for it.
    fn show_saving(&mut self, shown: be_wasm_root_api::SavingState) -> bool {
        let generation = self.saving.generation;
        if (be_wasm_root_api::SavingState {
            generation,
            ..shown.clone()
        }) == self.saving
        {
            return false;
        }
        self.saving = be_wasm_root_api::SavingState {
            generation: generation + 1,
            ..shown
        };
        let watching = std::mem::take(&mut self.saving_watchers);
        let answered = !watching.is_empty();
        for reply in watching {
            reply.send(&mut self.connection, &self.saving);
        }
        answered
    }

    /// Shows the root whether the host asks to keep display settings (`host.displays`), in
    /// place of what it showed. Answers whether a root's call waited for it.
    fn show_displays(&mut self, asking: Option<be_wasm_root_api::DisplayQuestion>) -> bool {
        if asking == self.displays.asking {
            return false;
        }
        self.displays = be_wasm_root_api::DisplayState {
            generation: self.displays.generation + 1,
            asking,
        };
        let watching = std::mem::take(&mut self.display_watchers);
        let answered = !watching.is_empty();
        for reply in watching {
            reply.send(&mut self.connection, &self.displays);
        }
        answered
    }

    /// Shows the root what the block stack shows the person (`host.core`), in place of what it
    /// showed: `shown`'s generation is the host's to count, so it is ignored. Answers whether a
    /// root's call waited for it.
    fn show_core(&mut self, shown: be_wasm_root_api::CoreView) -> bool {
        let generation = self.core.generation;
        if (be_wasm_root_api::CoreView {
            generation,
            ..shown.clone()
        }) == self.core
        {
            return false;
        }
        self.core = be_wasm_root_api::CoreView {
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

    /// What the root asked of the block stack since the last call, oldest first.
    fn take_core_commands(&mut self) -> Vec<be_wasm_root_api::CoreCommand> {
        std::mem::take(&mut self.core_commands)
    }

    /// What the root asked to make a secret since the last call, oldest first, each by the id
    /// `answer_secret` answers it by.
    fn take_secret_requests(&mut self) -> Vec<(u64, be_wasm_root_api::SecretCommand)> {
        std::mem::take(&mut self.secret_requests)
    }

    /// Answers the request of `id` with the secret it made, or with nothing if making it
    /// failed, and answers whether the root waited for it. An id it no longer waits for (one
    /// answered already, or the root stopped waiting) changes nothing, and the secret is
    /// dropped, which zeroes it.
    fn answer_secret(&mut self, id: u64, secret: Option<be_wasm_root_api::CoreSecret>) -> bool {
        let Some(at) = self
            .secret_replies
            .iter()
            .position(|(asked, _)| *asked == id)
        else {
            return false;
        };
        let (_, reply) = self.secret_replies.remove(at);
        reply.send(&mut self.connection, &secret);
        true
    }

    /// The blocks the root asked the editor of since the last call, each by the id
    /// `answer_editor` answers it by.
    fn take_editor_asks(&mut self) -> Vec<(u64, be_wasm_root_api::ShownBlock)> {
        std::mem::take(&mut self.editor_asks)
    }

    /// Answers the root's `host.editor-of` of `id`: the app and entry point that edit its
    /// block, or why none does. Answers whether the root waited for it.
    fn answer_editor(
        &mut self,
        id: u64,
        editor: Result<be_wasm_root_api::EditorApp, String>,
    ) -> bool {
        let Some(EditorReply::Of(reply)) = self.take_editor_reply(id) else {
            return false;
        };
        reply.send(&mut self.connection, &editor);
        true
    }

    /// The editors the root asked to open since the last call.
    fn take_openings(&mut self) -> Vec<EditorOpening> {
        std::mem::take(&mut self.openings)
    }

    /// Answers the root's `host.open-editor` of `id`: the editor opened, with the frames that
    /// open it, or why it was not. Answers whether the root waited for it.
    fn answer_opening(
        &mut self,
        id: u64,
        opened: Result<be_wasm_root_api::OpenedEditor, String>,
    ) -> bool {
        let Some(EditorReply::Open(reply)) = self.take_editor_reply(id) else {
            return false;
        };
        reply.send(&mut self.connection, &opened);
        true
    }

    fn take_editor_reply(&mut self, id: u64) -> Option<EditorReply> {
        let at = self
            .editor_replies
            .iter()
            .position(|(asked, _)| *asked == id)?;
        Some(self.editor_replies.remove(at).1)
    }

    /// What the root forwarded of the apps it spawned since the last call: each app's frames,
    /// by its instance, in order.
    fn take_forwarded(&mut self) -> Vec<(u64, Vec<Vec<u8>>)> {
        std::mem::take(&mut self.forwarded)
    }

    /// Sends the app the root spawned as `instance` `frames`, through the root. Answers whether
    /// the root waited for them.
    fn relay(&mut self, instance: u64, frames: Vec<Vec<u8>>) -> bool {
        if frames.is_empty() {
            return false;
        }
        match self
            .relaying
            .iter_mut()
            .find(|relayed| relayed.instance == instance)
        {
            Some(relayed) => relayed.frames.extend(frames),
            None => self
                .relaying
                .push(be_wasm_root_api::RelayedFrames { instance, frames }),
        }
        let Some(reply) = self.relay_watcher.take() else {
            return false;
        };
        reply.send(&mut self.connection, &std::mem::take(&mut self.relaying));
        self.unstepped = true;
        true
    }

    /// The children the root showed of the editors the host opened since the last call, the
    /// latest of each editor's view.
    fn take_shown_children(&mut self) -> Vec<ShownChildren> {
        std::mem::take(&mut self.shown_children)
    }

    /// What the root told the host's editors of their children since the last call
    /// (`host.tell-placer`): by the root's instance and the editor, in order.
    fn take_placer_events(&mut self) -> Vec<(u64, u64, be_wasm_root_api::PlacerEvent)> {
        std::mem::take(&mut self.placer_events)
    }

    /// The dock updates the root made for the host's editors since the last call
    /// (`host.dock`): by the root's instance and the editor, each encoded, in order.
    fn take_dock_updates(&mut self) -> Vec<(u64, u64, Vec<u8>)> {
        std::mem::take(&mut self.dock_updates)
    }

    /// Where the host's editors the root shows as placed children show their content since the
    /// last call (`host.place-content`), in order.
    fn take_placed_contents(&mut self) -> Vec<be_wasm_root_api::PlacedContent> {
        std::mem::take(&mut self.placed_contents)
    }

    /// Stops the root's sandbox instance `instance` for `why`, which the root hears from it.
    fn stop_instance(&mut self, instance: u64, why: &str) {
        self.sandbox.fail(instance, why);
        self.unstepped |= self.sandbox.poll(&mut self.connection);
    }

    /// The staged app the root's sandbox instance `instance` runs, while it runs: none once it
    /// stopped or the root let go of it.
    fn app_of(&self, instance: u64) -> Option<String> {
        self.sandbox.app_of(instance).map(str::to_owned)
    }

    /// Why the apps the root spawned have something in flight, if they do: frames wait between
    /// the root and the host, either way (`relay`, `take_forwarded`), the root has yet to be
    /// stepped with what its sandbox's `poll` or a relay answered it, or an instance has frames
    /// to step through or a step under way (`Server::unsettled`).
    #[cfg(not(target_arch = "wasm32"))]
    fn apps_unsettled(&self) -> Option<&'static str> {
        if !self.relaying.is_empty() {
            return Some("frames the host relayed wait for the root to take them");
        }
        if !self.forwarded.is_empty() {
            return Some("frames the root forwarded wait for the host to take them");
        }
        if self.unstepped {
            return Some("the root has yet to be stepped with what its apps or the host sent it");
        }
        self.sandbox.unsettled()
    }

    /// How the root answered the display questions since the last call, oldest first.
    fn take_display_answers(&mut self) -> Vec<(u64, bool)> {
        std::mem::take(&mut self.display_answers)
    }

    /// Whether the root asked to lock the session since the last call.
    fn take_lock_asked(&mut self) -> bool {
        std::mem::take(&mut self.lock_asked)
    }

    /// The password the root handed over since the last call, if it did, which the host checks
    /// and then shows the lock as it stands (`show_lock`).
    fn take_unlock(&mut self) -> Option<Password> {
        let unlock = self.unlock.take();
        self.taken |= unlock.is_some();
        unlock
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
        self.unstepped = false;
        self.guest.deliver(frames);
        self.guest.step()?;
        self.received()?;
        Ok(true)
    }

    /// Takes the frames the root sent.
    fn received(&mut self) -> Result<(), String> {
        for mut frame in self.guest.take_outbound() {
            self.connection
                .receive(&frame)
                .map_err(|error| format!("the root sent a frame the host cannot read: {error}"))?;
            // What it carried (a password) lives on only in what it was decoded into.
            be_wasm_ops::scrub(&mut frame);
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
                            self.unstepped |= self.sandbox.call(call, &mut self.connection);
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
                    } else if let Some(at) = self
                        .secret_replies
                        .iter()
                        .position(|(_, reply)| reply.op() == op)
                    {
                        let (id, reply) = self.secret_replies.remove(at);
                        reply.fail(&mut self.connection, OpError::Canceled);
                        // Not taken yet, the host never hears of it, so it makes no secret
                        // for nobody (an agent's credentials, which cannot be had again).
                        self.secret_requests.retain(|(asked, _)| *asked != id);
                    } else if let Some(at) = self
                        .editor_replies
                        .iter()
                        .position(|(_, reply)| reply.op() == op)
                    {
                        let (id, reply) = self.editor_replies.remove(at);
                        self.editor_asks.retain(|(asked, _)| *asked != id);
                        self.openings.retain(|opening| opening.id != id);
                        reply.fail(&mut self.connection, OpError::Canceled);
                    } else if self
                        .relay_watcher
                        .as_ref()
                        .is_some_and(|reply| reply.op() == op)
                    {
                        if let Some(reply) = self.relay_watcher.take() {
                            reply.fail(&mut self.connection, OpError::Canceled);
                        }
                    } else if let Some(at) = self
                        .display_watchers
                        .iter()
                        .position(|reply| reply.op() == op)
                    {
                        self.display_watchers
                            .remove(at)
                            .fail(&mut self.connection, OpError::Canceled);
                    } else if let Some(at) = self
                        .saving_watchers
                        .iter()
                        .position(|reply| reply.op() == op)
                    {
                        self.saving_watchers
                            .remove(at)
                            .fail(&mut self.connection, OpError::Canceled);
                    } else if let Some(at) =
                        self.lock_watchers.iter().position(|reply| reply.op() == op)
                    {
                        self.lock_watchers
                            .remove(at)
                            .fail(&mut self.connection, OpError::Canceled);
                    } else if let Some(at) = self
                        .statistics_watchers
                        .iter()
                        .position(|reply| reply.op() == op)
                    {
                        self.statistics_watchers
                            .remove(at)
                            .fail(&mut self.connection, OpError::Canceled);
                    } else if let Some(at) = self
                        .memory_watchers
                        .iter()
                        .position(|reply| reply.op() == op)
                    {
                        self.memory_watchers
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
        self.unstepped |= self.sandbox.poll(&mut self.connection);
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
            embed::Call::Host(embed::host::Call::BlockTypes { seen, reply }) => {
                match seen == self.block_types.generation {
                    true => self.block_types_watchers.push(reply),
                    false => reply.send(&mut self.connection, &self.block_types),
                }
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
                    for reply in std::mem::take(&mut self.memory_watchers) {
                        reply.send(&mut self.connection, &self.memory_history);
                    }
                }
            }
            embed::Call::Host(embed::host::Call::MemoryHistory { seen, reply }) => {
                match seen == self.memory_history.generation {
                    true => self.memory_watchers.push(reply),
                    false => reply.send(&mut self.connection, &self.memory_history),
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
            embed::Call::Host(embed::host::Call::Displays { seen, reply }) => {
                match seen == self.displays.generation {
                    true => self.display_watchers.push(reply),
                    false => reply.send(&mut self.connection, &self.displays),
                }
            }
            embed::Call::Host(embed::host::Call::Core { seen, reply }) => {
                match seen == self.core.generation {
                    true => self.core_watchers.push(reply),
                    false => reply.send(&mut self.connection, &self.core),
                }
            }
            embed::Call::Host(embed::host::Call::AskCore { command, reply }) => {
                self.core_commands.push(command);
                reply.send(&mut self.connection, &());
            }
            embed::Call::Host(embed::host::Call::MakeSecret { command, reply }) => {
                let id = self.next_secret;
                self.next_secret += 1;
                self.secret_requests.push((id, command));
                self.secret_replies.push((id, reply));
            }
            embed::Call::Host(embed::host::Call::AnswerDisplay { round, keep, reply }) => {
                // An answer to a round no longer asked (answered, withdrawn, or one the root
                // never saw) is dropped here, so the host hears only answers to its question.
                let asked = self.displays.asking.as_ref().map(|asking| asking.round);
                if asked == Some(round) {
                    self.display_answers.push((round, keep));
                }
                reply.send(&mut self.connection, &());
            }
            embed::Call::Host(embed::host::Call::Saving { seen, reply }) => {
                match seen == self.saving.generation {
                    true => self.saving_watchers.push(reply),
                    false => reply.send(&mut self.connection, &self.saving),
                }
            }
            embed::Call::Host(embed::host::Call::LockSession { reply }) => {
                self.lock_asked = true;
                reply.send(&mut self.connection, &());
            }
            embed::Call::Host(embed::host::Call::Unlock { password, reply }) => {
                reply.send(&mut self.connection, &());
                // One at a time, and only into a locked session the host is not making wait: any
                // other is dropped unchecked, which zeroes it. The host's wait after failed
                // checks (`retry-at`) it keeps itself.
                let busy = self.unlock.is_some() || self.taken || self.lock.checking;
                if self.lock.locked && !busy && self.lock.retry_at.is_none() {
                    self.unlock = Some(password);
                }
            }
            embed::Call::Host(embed::host::Call::LockPower { action, reply }) => {
                self.lock_powers.push(action);
                reply.send(&mut self.connection, &());
            }
            embed::Call::Host(embed::host::Call::EditorOf { shown, reply }) => {
                let id = self.next_editor;
                self.next_editor += 1;
                self.editor_asks.push((id, shown));
                self.editor_replies.push((id, EditorReply::Of(reply)));
            }
            embed::Call::Host(embed::host::Call::OpenEditor {
                instance,
                shown,
                reply,
            }) => {
                let id = self.next_editor;
                self.next_editor += 1;
                self.openings.push(EditorOpening {
                    id,
                    instance,
                    app: self.app_of(instance),
                    shown,
                });
                self.editor_replies.push((id, EditorReply::Open(reply)));
            }
            embed::Call::Host(embed::host::Call::Forward {
                instance,
                frames,
                reply,
            }) => {
                match self
                    .forwarded
                    .iter_mut()
                    .find(|(forwarded, _)| *forwarded == instance)
                {
                    Some((_, held)) => held.extend(frames),
                    None => self.forwarded.push((instance, frames)),
                }
                reply.send(&mut self.connection, &());
            }
            embed::Call::Host(embed::host::Call::Forwarded { reply }) => {
                match self.relaying.is_empty() {
                    true => {
                        if let Some(earlier) = self.relay_watcher.replace(reply) {
                            earlier.fail(&mut self.connection, OpError::Canceled);
                        }
                    }
                    false => reply.send(&mut self.connection, &std::mem::take(&mut self.relaying)),
                }
            }
            embed::Call::Host(embed::host::Call::ShowChildren {
                instance,
                editor,
                view,
                children,
                reply,
            }) => {
                self.shown_children.retain(|shown| {
                    (shown.instance, shown.editor, shown.view) != (instance, editor, view)
                });
                self.shown_children.push(ShownChildren {
                    instance,
                    editor,
                    view,
                    children,
                });
                reply.send(&mut self.connection, &());
            }
            embed::Call::Host(embed::host::Call::TellPlacer {
                instance,
                editor,
                event,
                reply,
            }) => {
                self.placer_events.push((instance, editor, event));
                reply.send(&mut self.connection, &());
            }
            embed::Call::Host(embed::host::Call::Dock {
                instance,
                editor,
                updates,
                reply,
            }) => {
                self.dock_updates.push((instance, editor, updates));
                reply.send(&mut self.connection, &());
            }
            embed::Call::Host(embed::host::Call::PlaceContent { placed, reply }) => {
                self.placed_contents.push(placed);
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
