//! be-host with a root guest in place of its own UI (`--root=APP`;
//! plans/host_as_platform_adapter.md, step 4e5c): the staged app `APP`'s entry point runs as the
//! root (be-wasm-root-host's `Root`) on the window's device, loaded and linked against the app
//! libraries as an app is (decisions/a_root_links_beui_as_an_app_does.md), with the fonts be-host
//! bundles (`be3:root`'s `host.fonts`). Each host frame hands it the
//! window's input since the last, asks it for a frame, and draws the frame's layer list: each
//! screen it names by handle (`Surface::Screen`) is the texture `Root::surface` resolves, drawn
//! as `beui::native::compose::quad` places it, and so is each surface of the root's own
//! (`Surface::Root`), which the root draws on the window's device as an app draws
//! (`Root::own_surface`); where it draws nothing the window shows its plain background. The host
//! draws on demand: when the root asks for its next frame (`frame-done`'s `next`) or has
//! something to step for (`Root::on_wake`). `drive paint` paints what the root's surfaces
//! painted (`be3:root`'s `paintings`) where its layer list shows them: its own, and the screens
//! of the apps it embeds, which it asks them for.
//!
//! This is the path step 4g turns on and makes the default, so it runs on be-host's real
//! startup path and window, through beui's runner; only what the window shows comes from the
//! root. The root takes the window's input as the platform delivered it, before beui makes
//! events of it (`Program::input`; step 4f1): beui's document hears none of it, so keys the root
//! does not route reach nothing in be-host. What the root asks of the window, its input method's
//! area, cursor shape, pointer lock and every sample of the pointer's moves, are the window's
//! requests (`frame-done`); each input it is handed carries the frame clock's time it was
//! sampled at, when the platform said.
//!
//! The host's own notices (be-host-base's `notices`: what of its own failed, or why it refused
//! something) reach the root through `be3:root`'s `host.notices`, which shows them as toasts
//! (plans/host_as_platform_adapter.md, step 4g3); what the person answers (`answer-notice`)
//! be-host does: dismissing the notice, copying its report, opening its link.
//!
//! Its documents' clipboard is the window's: what the root copies (`host.copy`) be-host puts on
//! it, and a paste it asks for (`host.paste`) be-host reads from it and answers with a later
//! frame. The platform's input method shows where the root says its focused field is, and what
//! it types reaches the root as input (plans/host_as_platform_adapter.md, step 4g4).
//!
//! While the root watches the host's statistics (`host.watch-statistics`), for its debug
//! panels, be-host measures its memory and hands the root its performance and memory rows
//! (`host.statistics`) as they change (plans/host_as_platform_adapter.md, step 4g5).
//!
//! It runs the host's core (`HostCore`), as be-host's own UI does: its data directory and the
//! lock that keeps a second be-host off it, the block stack, accounts and session, and the dev
//! workspace (`--dev-workspace`), each stepped every frame before the root's (step 4g7b2). The
//! core opens no editor of its own here: the root embeds them. It runs what the host keeps of the
//! platform as be-host's own UI does (step 4g7g; `platform`): the input devices and displays,
//! the login session (`BEUI_PLATFORM=drm`), and under it or `--desktop` the screen lock with its
//! password check and power actions, which the root shows (`host.lock`); the displays' keep
//! changes (`host.displays`); and under `--desktop` the desktop's OS services (notifications,
//! the tray, screenshots, media keys).
//!
//! The root is the app: one that fails to start or stops closes the window, and be-host exits
//! saying why, as it would on a panic.
//!
//! On the web (`?root=APP`; plans/host_as_platform_adapter.md, 4g7g2c4) the root runs on the
//! page (be-wasm-root-host's web `Root`): linked through app.js as an app is, it draws its own
//! surfaces on the page's gpu shim, and each app it spawns draws in a worker of its own. The page
//! shows each layer of its frame in a canvas of its own in `#layers`, above the host's canvas,
//! placed by CSS (`page`); the host's canvas, which beui draws, shows only the background. A
//! frame the root answers after its children draws once it has, from a later host frame. The
//! native-only parts, the desktop, program windows and the login session, are left out.

mod block_stack;
#[cfg(target_arch = "wasm32")]
mod page;
mod platform;
mod serving;
#[cfg(not(target_arch = "wasm32"))]
mod window;

use std::{
    cell::RefCell,
    error::Error,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use std::collections::HashMap;

use be_host_base::notices;
use be_host_desktop::desktop_host::{DesktopHost, Host};
use be_host_desktop::window_body::WindowStatus;
use be_host_runtime::{EditorBlock, Forwarded, HostChildStatus, InstanceRole};
use be_lib_layers::LayerList;
use be_lib_ui_types::ClipItem;
use be_lib_ui_types::input::Timed;
use be_sdk_api::{
    ChildId, ChildPreview, DialogStatus, EditorCapabilities, EditorInstanceId, HostWindowId,
    InteractionMode, ParentView, ResizeMode, ViewId, app::EditorEvent, shell_types::ShellMessage,
};
use be_sdk_app_host::pacing::FrameClock;
use be_sdk_manifest::AppManifest;
use be_wasm_root_api::{EditorApp, OpenedEditor, ShownPreview};
use be_wasm_root_api::{FrameRequest, Notice, NoticeAction, StatisticRow, WindowSize};
use be_wasm_root_host::EditorOpening;
use be_wasm_root_host::Root;
use beui::{CursorIcon, Input, WindowId};
#[cfg(target_arch = "wasm32")]
use page::Surfaces;
use serving::*;
use uuid::Uuid;
#[cfg(not(target_arch = "wasm32"))]
use window::Surfaces;

/// The id what the root shows has in the accessibility tree, `#root` to drive.
pub(crate) const SHOWN: &str = "root";

/// Runs `app`'s entry point as the root in a window opened with `options`, until the window
/// closes or the root stops, which is an error.
pub(crate) async fn run(
    app: String,
    core: crate::HostCore,
    options: beui_host::native::RunOptions,
) -> Result<(), Box<dyn Error>> {
    let stopped: Rc<RefCell<Option<String>>> = Rc::default();
    beui_host_window::run_program(options, RootShell::new(app, core, Rc::clone(&stopped))).await?;
    match stopped.take() {
        Some(why) => Err(why.into()),
        None => Ok(()),
    }
}

struct RootShell {
    app: String,
    /// What the host keeps (the plan's What the host keeps): its block stack, accounts and
    /// session, which it steps each frame and the root shows.
    core: crate::HostCore,
    document: beui::native::Document,
    /// What the window shows of the root's frames.
    surfaces: Surfaces,
    /// The windows' input since the root was last handed it, each in its window's points, with
    /// when the platform sampled it, if it said, and the files dragged over the window from
    /// outside it, in order.
    input: Vec<Taken>,
    root: Option<Root>,
    /// Where the frame clock's time the root is asked for starts.
    time_origin: Option<Instant>,
    /// Where the real time the frame deadlines are kept in starts.
    origin: Option<Instant>,
    /// When the root wants its next frame, in its time, as its last frame said.
    next: Option<Duration>,
    /// Whether the root owes work now, as its last frame said: be-host is busy while it does.
    owed: bool,
    /// The deadline each frame waits for the root's children until, as be-host's own UI
    /// keeps it (`FrameClock`): with drive's clock stopped, a long one, so which frame shows
    /// what a child drew does not depend on how fast the machine is.
    clock: FrameClock,
    /// The host's frames so far.
    pass: u64,
    /// Why the root stopped, once it did, for `run` to exit with.
    stopped: Rc<RefCell<Option<String>>>,
    /// What the root's last frame asked of the window beside its input method's area, which
    /// the window is asked again every frame.
    requests: Requests,
    /// Whether drive asked for a painting the root has not answered yet: be-host is busy until
    /// it has.
    painting: bool,
    /// Wakes the window for a frame, which asks the root for what drive asked.
    waker: Option<beui::native::Waker>,
    /// The generation of the host's notices the root was last handed.
    notices: Option<u64>,
    /// The catalog the root was last handed (`show_block_types`).
    block_types: HandedTypes,
    /// What the window's clipboard answered the root's pastes with, by their ids, until the
    /// root is handed them.
    pasted: Rc<RefCell<Vec<(u64, Option<ClipItem>)>>>,
    /// The memory rows the root was last handed while it watches the host's statistics.
    memory: Option<MemoryRows>,
    /// The editors be-host serves the root's apps, and the asks for them that wait.
    editors: Editors,
    /// The block stack as the root is shown it, and the secrets it asked for.
    block_stack: block_stack::BlockStack,
    /// The root's accessibility tree, grafted under the window's as an app's screen is, in a
    /// document of beui's the host reserves for it: what drive's `tree`, `find` and presses
    /// by name read under `--root`, and a screen reader hears.
    tree: be_sdk_app_host::accessibility::Graft,
    /// Whether the root was last asked for its tree (`frame-request.describe`).
    describing: bool,
    /// What the root was last shown of the platform: the screen lock and the displays.
    platform: platform::Platform,
    /// Whether the person did anything (a key, the pointer, a touch) since the last frame, which
    /// the root took before beui's document could hear it: the desktop host counts the time to
    /// lock the screen and turn the displays off from the document's input.
    active: bool,
    /// Whether the last frame was hurried (`Context::hurried`), as be-host's own UI keeps it:
    /// the block stack seals what its blocks owe at once, and `busy` waits for that.
    hurried: bool,
}

/// Grafts the tree the root's frame answered (`frame-done.accessibility`) under the window's,
/// where the root is drawn (`area`), as be-host grafts an app's screen. Actions on its
/// nodes (a screen reader's press) do not reach the root yet: drive presses by name at the
/// point its tree says, which the root hears as any press.
///
/// Published every frame once the root described itself, whether or not it sent a change, as
/// be-host publishes its apps' screens (be-host-runtime's `description`): a frame whose tree no
/// one took (nothing listened, `Runner::accessibility_requested`) still counts as known, so the
/// next listener's reset (`accessibility_known`) is answered with the whole graft, which holds
/// what that frame changed.
fn publish_tree(
    graft: &mut be_sdk_app_host::accessibility::Graft,
    context: &beui::native::Context,
    area: beui::Rect,
    tree: Option<&[u8]>,
) {
    let document = graft.document();
    // Actions on the root's nodes are not forwarded to it yet (the host adapter plan's Left).
    context.take_accessibility_actions(document);
    if let Some(tree) = tree.and_then(be_sdk_api::ops::decode::<be_sdk_api::accesskit::TreeUpdate>)
    {
        graft.update(tree);
    }
    if !graft.described() {
        return;
    }
    let grafted = graft.grafted(
        be_sdk_app_host::accessibility::Placing {
            title: None,
            area,
            origin: area.min.to_vec2(),
            scale: 1.0,
        },
        true,
        !context.accessibility_known(document),
    );
    context.publish_accessibility(
        document,
        beui::native::accessibility::Fragment::new(grafted.nodes, grafted.root, grafted.focus),
    );
}

/// What a root's frame asks of its window (`frame-done`'s `cursor`, `pointer-locked` and
/// `every-sample`).
#[derive(Clone, Copy, Default)]
struct Requests {
    cursor: CursorIcon,
    pointer_locked: bool,
    every_sample: bool,
}

impl RootShell {
    fn new(app: String, core: crate::HostCore, stopped: Rc<RefCell<Option<String>>>) -> Self {
        // The root draws the one command palette over every app, as be-host's own UI does, so
        // the editors it opens draw none of their own (`open-block.host-palette`).
        be_host_runtime::draw_palette(true);
        let (document, surfaces) = Surfaces::document(core.desktop);
        Self {
            app,
            core,
            document,
            surfaces,
            input: Vec::new(),
            root: None,
            time_origin: None,
            origin: None,
            next: None,
            owed: false,
            clock: FrameClock::default(),
            pass: 0,
            stopped,
            requests: Requests::default(),
            painting: false,
            waker: None,
            notices: None,
            block_types: HandedTypes::default(),
            pasted: Rc::default(),
            memory: None,
            editors: Editors::default(),
            block_stack: block_stack::BlockStack::default(),
            tree: be_sdk_app_host::accessibility::Graft::new(
                beui::native::accessibility::next_document_id(),
            ),
            describing: false,
            platform: platform::Platform::default(),
            active: false,
            hurried: false,
        }
    }

    /// The root stopped, or never started, for `why`: it draws nothing again, and be-host exits
    /// saying so once the window closes, which it asks for at the end of the frame.
    fn stopped(&mut self, why: &str) {
        let why = format!("the root {} stopped: {why}", self.app);
        eprintln!("be-host: {why}");
        self.stopped.borrow_mut().get_or_insert(why);
        #[cfg(target_arch = "wasm32")]
        self.surfaces.clear(self.root.as_ref());
        self.root = None;
        self.next = None;
        self.owed = false;
        self.requests = Requests::default();
    }

    /// Hands the root its windows' input since it was last handed it, in order, a call for each
    /// run of one window's, each input with the frame clock's time it was sampled at, counted
    /// from `origin` as the frames' are. A sample's time is the platform's real instant, so
    /// while drive's clock is stopped it can be later than the frame's. The main window is the one the layer list names as none;
    /// the host opens no other for the root yet.
    fn deliver_input(&mut self, origin: Instant) {
        let input = std::mem::take(&mut self.input);
        let Some(root) = &mut self.root else {
            return;
        };
        for run in input.chunk_by(|left, right| match (left, right) {
            (Taken::Input(left, ..), Taken::Input(right, ..)) => left == right,
            _ => false,
        }) {
            let handed = match &run[0] {
                // Files dragged over the window: the platform names no window, and the host
                // opens the root one.
                Taken::Files(drag) => root.drag_files(None, drag.clone()),
                Taken::Input(window, ..) => {
                    let window = (!window.is_main()).then(|| window.raw());
                    let events: Vec<Timed> = run
                        .iter()
                        .filter_map(|taken| match taken {
                            Taken::Input(_, input, time) => Some(Timed {
                                input: input.clone(),
                                time: time.map(|at| nanos(at.saturating_duration_since(origin))),
                            }),
                            Taken::Files(_) => None,
                        })
                        .collect();
                    root.input(window, be_lib_ui_types::input::encode(&events))
                }
            };
            if let Err(why) = handed {
                return self.stopped(&why);
            }
        }
    }

    /// Asks the root for a frame for `time` and shows its layer list.
    fn frame(&mut self, context: &beui::native::Context, time: Duration, area: beui::Rect) {
        // On the page the root loads asynchronously, and is asked for frames once it has.
        // A root that loaded after the host stopped it (`stopped`) is let go.
        #[cfg(target_arch = "wasm32")]
        if self.root.is_none() && self.stopped.borrow().is_none() {
            match self.surfaces.loaded() {
                Some(Ok(root)) => self.root = Some(root),
                Some(Err(why)) => return self.stopped(&format!("it did not start: {why}")),
                None => {}
            }
        }
        let Some(root) = &mut self.root else {
            return;
        };
        let origin = *self.origin.get_or_insert_with(Instant::now);
        let since = |at: Instant| at.saturating_duration_since(origin);
        self.clock.begin_frame(
            context.next_present().map(since),
            context.refresh_interval(),
            context.last_prepared().map(since),
        );
        self.pass += 1;
        if self.painting
            && let Err(why) = root.ask_paintings()
        {
            return self.stopped(&why);
        }
        let deadline = origin
            + self
                .clock
                .deadline(self.pass, context.clock_stopped(), since(Instant::now()));
        show_notices(root, &mut self.notices);
        show_block_types(&self.core, root, &mut self.block_types);
        show_statistics(root, &mut self.memory);
        for (id, item) in self.pasted.take() {
            root.answer_paste(id, item.as_ref().map(be_sdk_api::ops::encode));
        }
        // As be-host describes its own apps' screens (be-host-runtime's `describe`): a driver
        // or a screen reader asks for the tree. A frame follows the one that starts it, which
        // the root answers with its whole tree once its surfaces and apps described theirs.
        let describing = context.accessibility_active()
            && (context.automated() || context.accessibility_requested());
        if describing && !self.describing {
            context.request_repaint();
        }
        self.describing = describing;
        let request = FrameRequest {
            time: nanos(time),
            wall_clock: nanos(beui::reactive::wall_clock()),
            utc_offset: be_host_base::platform::utc_offset(),
            preferences: be_sdk_api::ops::encode(&context.preferences()),
            window: WindowSize {
                width: area.width(),
                height: area.height(),
                pixels_per_point: context.pixels_per_point(),
            },
            describe: describing,
            // As be-host tells its own apps: what the session's shell said of its layout.
            phone: self
                .editors
                .served
                .values()
                .any(be_host_runtime::Forwarded::phone_layout),
        };
        #[cfg(not(target_arch = "wasm32"))]
        let done = root.frame(request, deadline);
        // On the page the root is stepped as far as it goes and answers once its children did,
        // which may be after this frame: what it answered then is shown from the next, which
        // its answer wakes the host for. A frame it is still drawing is left to finish.
        #[cfg(target_arch = "wasm32")]
        let Some(done) = self
            .surfaces
            .frame(root, request, deadline, self.waker.clone())
        else {
            // The window's tree holds the root's as its last frame described it meanwhile: beui
            // drops a document no frame publishes, and the next answer would resend the whole tree.
            publish_tree(&mut self.tree, context, area, None);
            return;
        };
        let done = match done {
            Ok(done) => done,
            Err(why) => return self.stopped(&why),
        };
        self.served(context, time, area, done);
    }

    /// Serves what the root's frame `done` asked of the host, and shows its layer list.
    fn served(
        &mut self,
        context: &beui::native::Context,
        time: Duration,
        area: beui::Rect,
        done: be_wasm_root_api::FrameDone,
    ) {
        let Some(root) = &mut self.root else {
            return;
        };
        // What the root's router delivered to the program windows it placed, which each hears
        // as its view would have caught it.
        if self.core.desktop {
            let clock = self.time_origin.unwrap_or_else(|| context.now());
            for delivered in &done.host_input {
                let events = match be_lib_ui_types::input::decode(&delivered.events) {
                    Ok(events) => events,
                    Err(why) => {
                        return self
                            .stopped(&format!("its input for a window cannot be read: {why}"));
                    }
                };
                let input = events
                    .into_iter()
                    .map(|timed| {
                        let at = timed
                            .time
                            .map_or_else(|| context.now(), |at| clock + Duration::from_nanos(at));
                        (timed.input, at)
                    })
                    .collect();
                Host::deliver(HostWindowId(delivered.surface), input);
            }
        }
        publish_tree(&mut self.tree, context, area, done.accessibility.as_deref());
        clipboard(context, root, &self.pasted);
        serve_editors(&mut self.core, root, &mut self.editors);
        for (id, data) in std::mem::take(&mut self.editors.artifact_applies) {
            self.core.store_artifact_settings(id, data);
        }
        // The screen lock and the displays' keep changes, what the host keeps of the platform.
        self.platform.serve(&mut self.core, root, time);
        // What the root asked of the block stack in this frame, done at once; what that changed
        // reaches it with its next frame, which showing it wakes the host for.
        self.block_stack.step(&mut self.core, root, context);
        let answers = root.take_answers();
        if !answers.is_empty() {
            answer_notices(context, answers);
            // Dismissing one changes them: the root hears it with its next frame, which this
            // wakes the host for.
            show_notices(root, &mut self.notices);
        }
        if let Some(painted) = root.take_paintings() {
            self.painting = false;
            self.surfaces.painted(&self.app, painted);
        }
        self.next = done.next.map(Duration::from_nanos);
        self.owed = done.owed;
        self.requests = Requests {
            cursor: CursorIcon::from_name(&done.cursor).unwrap_or_default(),
            pointer_locked: done.pointer_locked,
            every_sample: done.every_sample,
        };
        let ime = match done.ime.as_deref().map(beui::ImeArea::decode).transpose() {
            Ok(ime) => ime,
            Err(why) => return self.stopped(&why),
        };
        // The window shows the platform's input method where the root's focused surface edits.
        context.set_ime_area(ime);
        let list = match LayerList::decode(&done.layers) {
            Ok(list) => list,
            Err(why) => return self.stopped(&format!("its layer list cannot be read: {why}")),
        };
        // The host has one window, which the list names as none.
        #[cfg(not(target_arch = "wasm32"))]
        self.surfaces
            .show(self.core.desktop, root, list, &mut self.document);
        #[cfg(target_arch = "wasm32")]
        if let Err(why) = self.surfaces.show(root, list, context) {
            self.stopped(&why);
        }
    }
}

impl beui::native::Program for RootShell {
    fn setup(&mut self, setup: &beui::native::Setup) {
        self.waker = Some(setup.waker.clone());
        // A notice shown wakes the window, whose next frame hands it to the root.
        let waker = setup.waker.clone();
        be_host_base::host::install_waker(move || waker.wake());
        be_host_base::host::install_clock(beui::reactive::now);
        // The host's own apps the core runs (its blocks' host components) draw on the window's
        // device too.
        be_host_runtime::install(setup, be_host_audio::open);
        if self.core.desktop {
            Host::start(setup);
        }
        self.core.start_platform(setup);
        #[cfg(not(target_arch = "wasm32"))]
        match window::start(&self.app, setup) {
            Ok(root) => self.root = Some(root),
            Err(why) => self.stopped(&format!("it did not start: {why}")),
        }
        #[cfg(target_arch = "wasm32")]
        if let Err(why) = self.surfaces.load(&self.app, setup.waker.clone()) {
            self.stopped(&format!("it did not start: {why}"));
        }
    }

    fn renderer_replaced(&mut self, _setup: &beui::native::Setup) {
        // What the root's children drew is on the device it started on, which is gone.
        if self.root.is_some() {
            self.stopped("the window's device was replaced, and the root cannot move to another");
        }
    }

    fn update(&mut self, context: &beui::native::Context, rect: beui::Rect) {
        // The frame clock's time, which stands still while drive's clock is stopped.
        let now = context.now();
        let origin = *self.time_origin.get_or_insert(now);
        let time = now.saturating_duration_since(origin);
        // The host's frame, as be-host's own UI begins it (`Shell::update`): its clock, which the
        // screen lock and the displays' question count on. It is not ended as be-host's own UI
        // ends it (`frame::end`): what the core asks of the window is read below.
        crate::frame::begin(context);
        if std::mem::take(&mut self.active) {
            self.document.note_input();
        }
        self.hurried = context.hurried();
        if self.hurried {
            be_host_stack::seal_owed();
        }
        // The desktop host before the core, as be-host's own UI steps them (`Shell::update`): a
        // window it took in this frame is in the list the core publishes in it.
        if self.core.desktop {
            Host::before(context, rect, &mut self.document);
        }
        self.core.frame();
        // The display settings' preferences (the theme among them) over the system's, as
        // be-host's own UI applies them (`Shell::update`): the root is told them as resolved.
        self.core.sync_ui_settings();
        if let Some(preferences) = self.core.preferences.take() {
            context.set_preference_layer(beui::PreferenceSource::Program, preferences);
        }
        // Programs own their windows' input while the session's editor shows, as be-host's own
        // UI says while it shows the organization's screen.
        let owned = self.core.desktop && self.core.root_session.is_some();
        be_host_desktop::desktop_host::configure(|desktop| desktop.owned = owned);
        // The apps be-host runs itself (an artifact's exporter making it again, its blocks' host
        // components) start and are asked for their frames as be-host's own window asks them,
        // once it laid out (`on_interacted`, `on_laid_out`): the root's apps are the root's.
        be_host_runtime::start_frames(false);
        // Whether their answers change what be-host lays out does not matter here: under the
        // root be-host's window shows none of their views, only what the root drew.
        let _relaid = be_host_runtime::settle();
        // What the host's apps did in this frame's `poll` reaches whoever waits on it.
        beui::native::enter(&mut self.document, be_host_base::host::deliver);
        self.deliver_input(origin);
        self.frame(context, time, rect);
        self.document.show(context, rect);
        if self.core.desktop {
            Host::after(context, &mut self.document);
        }
        // After the document, whose own requests (it hears no input, so the default cursor) the
        // root's replace.
        let Requests {
            cursor,
            pointer_locked,
            every_sample,
        } = self.requests;
        context.set_cursor_icon(cursor);
        context.set_pointer_locked(pointer_locked);
        if every_sample {
            context.want_every_sample(WindowId::MAIN);
        }
        // The core closes the window once the session it was asked to end has ended (its
        // programs' windows closed first, `close_requested`), as be-host's own frame closes it
        // (`frame::end`). Its frames come as the root's do, and as what it waits on wakes it
        // (`host::wake`): the repaints it asks for, which its own UI's frames answer, are not
        // the root's, which asks for its own (`next`).
        if self.stopped.borrow().is_some() || be_host_base::host::finish().close {
            context.close_window();
            return;
        }
        match self.next {
            Some(at) if at <= time => context.request_repaint(),
            Some(at) => context.request_repaint_after(at - time),
            None => {}
        }
        // The frames the screen lock and the displays' question need of their own.
        if let Some(due) = self.platform.due() {
            context.request_repaint_after(due);
        }
    }

    /// The root takes all of its windows' input, which it routes to its surfaces itself; beui's
    /// document, which only shows what the root drew, hears none of it.
    fn input(&mut self, window: WindowId, input: &Input, time: Option<Instant>) -> bool {
        self.active |= activity(input);
        self.input.push(Taken::Input(window, input.clone(), time));
        true
    }

    /// The root takes files dragged over the window from outside it too, which it carries to
    /// the view under the pointer: dropped, each with what it holds, read here on the frame's
    /// thread (as be-host's own views read them, be-host-runtime's `file_drop`); those dropped
    /// together reach it together.
    fn file_drag(&mut self, drag: &beui::FileDrag) -> bool {
        use be_wasm_root_api::input::{DroppedFile, FileDrag};
        let drag = match drag {
            beui::FileDrag::Hovered => FileDrag::Hovered,
            beui::FileDrag::Cancelled => FileDrag::Cancelled,
            // One that cannot be read (a folder, or one it may not read) is left out, but the
            // drop is handed on all the same, so the root ends the drag.
            beui::FileDrag::Dropped(file) => {
                let dropped =
                    be_host_runtime::read_dropped(file.clone()).map(|dropped| DroppedFile {
                        name: dropped.name,
                        data: dropped.data,
                    });
                if let Some(Taken::Files(FileDrag::Dropped(held))) = self.input.last_mut() {
                    held.extend(dropped);
                    return true;
                }
                FileDrag::Dropped(dropped.into_iter().collect())
            }
        };
        self.input.push(Taken::Files(drag));
        true
    }

    /// Only what the root owes now: a frame it wants later, or for an animation, is the repaint
    /// it asked for, which drive tells apart itself. A painting drive asked for is owed until
    /// the root answers it.
    fn busy(&self) -> Vec<String> {
        let mut busy = match self.core.error.is_some() {
            true => Vec::new(),
            false => self.core.busy(self.hurried),
        };
        busy.extend(be_host_runtime::busy());
        busy.extend(Host::busy());
        if self.core.root_session_pending {
            busy.push("the session's block is not listed yet".to_owned());
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(why) = self.root.as_ref().and_then(Root::apps_unsettled) {
            busy.push(format!("the root's apps are not settled: {why}"));
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(why) = self.surfaces.busy(self.root.as_ref()) {
            busy.push(format!("the root {}: {why}", self.app));
        }
        if self
            .editors
            .served
            .values()
            .any(be_host_runtime::Forwarded::awaiting_content)
        {
            busy.push("an editor the root shows waits for a block's content".to_owned());
        }
        if !self.editors.asks.is_empty() || !self.editors.openings.is_empty() {
            busy.push("the root asked for an editor of a block not listed yet".to_owned());
        }
        if !self.editors.finds.is_empty() {
            busy.push("the root's quick open waits for the search index".to_owned());
        }
        if self.owed {
            busy.push(format!("the root {} owes a frame", self.app));
        }
        if self.painting && self.root.is_some() {
            busy.push(format!("the root {} owes a painting", self.app));
        }
        busy
    }

    fn out_of_step(&self) -> Option<Vec<String>> {
        Some(self.core.out_of_step())
    }

    fn connected(&self) -> Option<bool> {
        Some(be_host_stack::status().connected)
    }

    fn in_step_due(&self) -> Option<Instant> {
        be_host_stack::modules_due()
    }

    fn close_requested(&mut self) -> bool {
        self.core.close_requested()
    }

    fn lifecycle(&mut self, change: beui::native::Lifecycle) {
        match change {
            beui::native::Lifecycle::Background => be_host_stack::backgrounded(),
            beui::native::Lifecycle::Foreground
            | beui::native::Lifecycle::Focused
            | beui::native::Lifecycle::NetworkChanged => be_host_stack::retry_now(),
        }
    }

    /// drive asks what the window shows: the root is asked what its own surfaces painted with
    /// the next frame, which this wakes the window for.
    fn request_painting(&mut self) {
        self.painting = true;
        if let Some(waker) = &self.waker {
            waker.wake();
        }
    }

    /// What a program `drive exec` starts is given to reach the desktop host's compositor, as
    /// under be-host's own UI: nothing without `--desktop`.
    fn environment(&self) -> Vec<(String, String)> {
        match self.core.desktop {
            true => Host::environment(),
            false => Vec::new(),
        }
    }

    fn holdable(&self) -> Vec<String> {
        be_host_runtime::running()
            .into_iter()
            .map(|status| status.app_id)
            .collect()
    }

    fn exiting(&mut self) {
        // Stops the root and every instance it spawned first, which draw on the window's device
        // and are served by the host; then the desktop host and be-host's own apps, as be-host's
        // own UI stops them (`Shell::exiting`).
        #[cfg(target_arch = "wasm32")]
        self.surfaces.clear(self.root.as_ref());
        self.root = None;
        if self.core.desktop {
            Host::exiting();
        }
        be_host_runtime::exiting();
        self.core.stop_platform();
    }
}

/// Whether `input` is the person doing something, as beui's document counts its input frames
/// (`Document::note_input`).
fn activity(input: &Input) -> bool {
    matches!(
        input,
        Input::Key(_)
            | Input::Text(_)
            | Input::Paste(_)
            | Input::PointerMoved(_)
            | Input::PointerMotion(_)
            | Input::PointerButton { .. }
            | Input::Scroll(_)
            | Input::Zoom(_)
            | Input::Touchpad(_)
            | Input::Touch { .. }
            | Input::Pen { .. }
    )
}

/// What `RootShell` hands the root in order: an input in a window, or files dragged over the
/// window from outside it.
enum Taken {
    Input(WindowId, Input, Option<Instant>),
    Files(be_wasm_root_api::input::FileDrag),
}

/// The manifest of the staged app `app`, which must say it is a root.
fn root_manifest(app: &str) -> Result<Arc<AppManifest>, String> {
    let apps = be_host_stack::discovery::apps();
    let manifest = apps
        .manifests()
        .iter()
        .find(|manifest| manifest.identity.id == app)
        .cloned()
        .ok_or_else(|| {
            let mut why = format!("no staged app has the id {app}");
            for error in apps.errors() {
                why.push_str(&format!("; {error}"));
            }
            why
        })?;
    if !manifest.root {
        return Err(format!(
            "{app} is an app, not a root: its manifest does not say \"root\": true"
        ));
    }
    Ok(manifest)
}
