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
//! core opens no editor of its own here: the root embeds them. It runs no screen lock and none
//! of the desktop's OS services: step 4g7g brings back the rest of what the host keeps (the
//! plan's What the host keeps) before the root becomes the default; until then `--root` is
//! refused as a login session (`BEUI_PLATFORM=drm`), which would have no lock screen, and with
//! `--desktop`.
//!
//! The root is the app: one that fails to start or stops closes the window, and be-host exits
//! saying why, as it would on a panic.

mod block_stack;

use std::{
    cell::RefCell,
    collections::BTreeMap,
    error::Error,
    rc::Rc,
    time::{Duration, Instant},
};

use std::collections::HashMap;

use be_host_base::notices;
use be_host_runtime::{EditorBlock, Forwarded, HostChildStatus, InstanceRole};
use be_lib_layers::{Layer, LayerList, Surface};
use be_lib_ui_types::ClipItem;
use be_lib_ui_types::input::Timed;
use be_sdk_api::{
    ChildId, ChildPreview, DialogStatus, EditorCapabilities, EditorInstanceId, InteractionMode,
    ParentView, ResizeMode, ViewId, app::EditorEvent, shell_types::ShellMessage,
};
use be_sdk_app_host::pacing::FrameClock;
use be_wasm_root_api::{EditorApp, OpenedEditor, ShownPreview};
use be_wasm_root_api::{FrameRequest, Notice, NoticeAction, StatisticRow, WindowSize};
use be_wasm_root_host::EditorOpening;
use be_wasm_root_host::Root;
use beui::native::painting::{Painting, Paints, Placement, ScreenPaintings};
use beui::native::texture_quad::{Pipelines, Quad, Quads};
use beui::native::{Draw, DrawAt, Drawing as GpuDrawing, compose, draw_gpu};
use beui::reactive::*;
use beui::styled::*;
use beui::{CursorIcon, Input, WindowId};
use uuid::Uuid;

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
    /// What the window shows of the root's last frame.
    shown: Signal<Option<GpuDrawing>>,
    /// The windows' input since the root was last handed it, each in its window's points, with
    /// when the platform sampled it, if it said.
    input: Vec<(WindowId, Input, Option<Instant>)>,
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
    /// What the last frame showed, so an unchanged frame keeps its drawing.
    drawn: Vec<(Layer, u64)>,
    /// What the root's last frame asked of the window beside its input method's area, which
    /// the window is asked again every frame.
    requests: Requests,
    /// What the root's own surfaces painted when last asked (drive's `paint`), by their ids.
    paintings: Rc<RefCell<BTreeMap<u64, Painting>>>,
    /// Whether drive asked for a painting the root has not answered yet: be-host is busy until
    /// it has.
    painting: bool,
    /// Wakes the window for a frame, which asks the root for what drive asked.
    waker: Option<beui::native::Waker>,
    /// The generation of the host's notices the root was last handed.
    notices: Option<u64>,
    /// The catalog the root was last handed (`show_block_types`).
    block_types: Option<HandedTypes>,
    /// What the window's clipboard answered the root's pastes with, by their ids, until the
    /// root is handed them.
    pasted: Rc<RefCell<Vec<(u64, Option<ClipItem>)>>>,
    /// The memory rows the root was last handed while it watches the host's statistics.
    memory: Option<MemoryRows>,
    /// The editors be-host serves the root's apps, and the asks for them that wait.
    editors: Editors,
    /// The block stack as the root is shown it, and the secrets it asked for.
    block_stack: block_stack::BlockStack,
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
        let mut shown = None;
        let document = beui_host::native::build(|| {
            let drawing = create_signal(None::<GpuDrawing>);
            shown = Some(drawing);
            view! {
                <beui::styled::Root title="be-host">
                    <Drawing @test_id={SHOWN} draw={move || draw_gpu(drawing.get())} />
                </beui::styled::Root>
            }
        });
        Self {
            app,
            core,
            document,
            shown: shown.expect("the drawing's signal is made while the document is built"),
            input: Vec::new(),
            root: None,
            time_origin: None,
            origin: None,
            next: None,
            owed: false,
            clock: FrameClock::default(),
            pass: 0,
            stopped,
            drawn: Vec::new(),
            requests: Requests::default(),
            paintings: Rc::default(),
            painting: false,
            waker: None,
            notices: None,
            block_types: None,
            pasted: Rc::default(),
            memory: None,
            editors: Editors::default(),
            block_stack: block_stack::BlockStack::default(),
        }
    }

    /// The root stopped, or never started, for `why`: it draws nothing again, and be-host exits
    /// saying so once the window closes, which it asks for at the end of the frame.
    fn stopped(&mut self, why: &str) {
        let why = format!("the root {} stopped: {why}", self.app);
        eprintln!("be-host: {why}");
        self.stopped.borrow_mut().get_or_insert(why);
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
        for run in input.chunk_by(|(left, ..), (right, ..)| left == right) {
            let window = run[0].0;
            let window = (!window.is_main()).then(|| window.raw());
            let events: Vec<Timed> = run
                .iter()
                .map(|(_, input, time)| Timed {
                    input: input.clone(),
                    time: time.map(|at| nanos(at.saturating_duration_since(origin))),
                })
                .collect();
            if let Err(why) = root.input(window, be_lib_ui_types::input::encode(&events)) {
                return self.stopped(&why);
            }
        }
    }

    /// Asks the root for a frame for `time` and shows its layer list.
    fn frame(&mut self, context: &beui::native::Context, time: Duration, area: beui::Rect) {
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
        let request = FrameRequest {
            time: nanos(time),
            wall_clock: nanos(beui::reactive::wall_clock()),
            utc_offset: be_host_base::platform::utc_offset(),
            window: WindowSize {
                width: area.width(),
                height: area.height(),
                pixels_per_point: context.pixels_per_point(),
            },
        };
        let done = match root.frame(request, deadline) {
            Ok(done) => done,
            Err(why) => return self.stopped(&why),
        };
        clipboard(context, root, &self.pasted);
        serve_editors(&mut self.core, root, &mut self.editors);
        refuse_unserved(root);
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
            // A painting that cannot be read paints nothing: it is for the tests, not the user.
            let painted = match painted.is_empty() {
                true => ScreenPaintings::default(),
                false => ScreenPaintings::decode(&painted).unwrap_or_else(|why| {
                    eprintln!(
                        "be-host: the root {}'s paintings cannot be read: {why}",
                        self.app
                    );
                    ScreenPaintings::default()
                }),
            };
            *self.paintings.borrow_mut() = painted.screens.into_iter().collect();
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
        let mut layers = Vec::new();
        for layer in list
            .windows
            .into_iter()
            .filter(|window| window.window.is_none())
            .flat_map(|window| window.layers)
            .filter(|layer| layer.shown)
        {
            let shown = match layer.surface {
                Surface::Screen { instance, surface } | Surface::Anchored { instance, surface } => {
                    root.surface(instance, surface).cloned()
                }
                Surface::Root(surface) => root.own_surface(surface).cloned(),
                // The host owns no surfaces of its own here.
                Surface::Host(_) => None,
            };
            if let Some(shown) = shown {
                layers.push((layer, shown.generation, shown.texture));
            }
        }
        let drawn: Vec<(Layer, u64)> = layers
            .iter()
            .map(|(layer, generation, _)| (layer.clone(), *generation))
            .collect();
        if drawn == self.drawn {
            return;
        }
        self.drawn = drawn;
        let drawing = (!layers.is_empty()).then(|| {
            beui::native::painted_drawing(ShownLayers {
                layers: layers
                    .into_iter()
                    .map(|(layer, _, texture)| (layer, texture))
                    .collect(),
                quads: Quads::default(),
                paintings: Rc::clone(&self.paintings),
            })
        });
        let shown = self.shown;
        beui::native::enter(&mut self.document, || shown.set(drawing));
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
        be_host_runtime::install(setup);
        match start(&self.app, setup) {
            Ok(root) => self.root = Some(root),
            Err(why) => self.stopped(&format!("it did not start: {why}")),
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
        self.core.frame();
        // What the host's apps did in this frame's `poll` reaches whoever waits on it.
        beui::native::enter(&mut self.document, be_host_base::host::deliver);
        self.deliver_input(origin);
        self.frame(context, time, rect);
        self.document.show(context, rect);
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
        if self.stopped.borrow().is_some() {
            context.close_window();
            return;
        }
        match self.next {
            Some(at) if at <= time => context.request_repaint(),
            Some(at) => context.request_repaint_after(at - time),
            None => {}
        }
    }

    /// The root takes all of its windows' input, which it routes to its surfaces itself; beui's
    /// document, which only shows what the root drew, hears none of it.
    fn input(&mut self, window: WindowId, input: &Input, time: Option<Instant>) -> bool {
        self.input.push((window, input.clone(), time));
        true
    }

    /// Only what the root owes now: a frame it wants later, or for an animation, is the repaint
    /// it asked for, which drive tells apart itself. A painting drive asked for is owed until
    /// the root answers it.
    fn busy(&self) -> Vec<String> {
        let mut busy = match self.core.error.is_some() {
            true => Vec::new(),
            false => self.core.busy(false),
        };
        busy.extend(be_host_runtime::busy());
        if self.core.root_session_pending {
            busy.push("the session's block is not listed yet".to_owned());
        }
        if let Some(why) = self.root.as_ref().and_then(Root::apps_unsettled) {
            busy.push(format!("the root's apps are not settled: {why}"));
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

    fn exiting(&mut self) {
        // Stops the root and every instance it spawned.
        self.root = None;
    }
}

/// Starts `app`'s entry point as the root on the window's device, which it and its staged apps,
/// found as be-host finds every app's, draw on, waking the host whenever it has something to
/// step for.
fn start(app: &str, setup: &beui::native::Setup) -> Result<Root, String> {
    let gpu = setup
        .get::<beui_renderer_wgpu::present::GpuSetup>()
        .ok_or("the window has no GPU renderer, and the root's apps draw on its device")?;
    let apps = be_host_stack::discovery::apps();
    let manifest = apps
        .manifests()
        .iter()
        .find(|manifest| manifest.identity.id == app)
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
    // Loaded as an app is, linked against the libraries it names, which every app on this
    // device shares one compile of; but stepped inline on the window's thread, never on a
    // thread of its own.
    let host = be_wasm_host::Host::new(gpu.device.clone(), gpu.queue.clone())?;
    let module = be_host_runtime::staged_app(app, &manifest.entry_point)?.load(&host)?;
    let root = Root::start(
        module,
        Box::new(be_host_runtime::staged_app),
        gpu.device.clone(),
        gpu.queue.clone(),
    )?
    .with_fonts(fonts());
    let waker = setup.waker.clone();
    root.on_wake(move || waker.wake());
    Ok(root)
}

/// The catalog a root was handed: the registry's block types and the programs' editors listed.
struct HandedTypes {
    types: std::sync::Arc<be_host_runtime::AppCatalog>,
    programs: u64,
}

/// Hands the root the host's catalog once it changed since it was `handed` (`host.block-types`):
/// the registry's block types, templates and editors with the editors of the programs listed,
/// as be-host-runtime tells its own apps, which name and draw block types from it.
fn show_block_types(core: &crate::HostCore, root: &mut Root, handed: &mut Option<HandedTypes>) {
    let types = core.registry.app_block_types();
    let seen = handed
        .as_ref()
        .filter(|handed| std::sync::Arc::ptr_eq(&handed.types, types))
        .map(|handed| handed.programs);
    let Some((programs, program_editors)) = be_host_base::program_editors::listed_since(seen)
    else {
        return;
    };
    let mut catalog = types.catalog.clone();
    catalog.editors.extend(program_editors);
    root.show_block_types(
        be_sdk_api::ops::encode(&catalog),
        be_sdk_api::ops::encode(&types.debug_templates),
    );
    *handed = Some(HandedTypes {
        types: std::sync::Arc::clone(types),
        programs,
    });
}

/// Hands `root` the host's notices if they changed since the generation it was handed, `seen`.
fn show_notices(root: &mut Root, seen: &mut Option<u64>) {
    let generation = notices::generation();
    if *seen != Some(generation) {
        *seen = Some(generation);
        root.show_notices(shown_notices());
    }
}

/// Hands `root` the host's statistics while it watches them (`host.watch-statistics`), and
/// measures be-host's memory only then, writing it nowhere: the sampling thread parks while
/// nothing watches and wakes the window for each sample while the root does, and each frame
/// hands the root what changed.
fn show_statistics(root: &mut Root, memory: &mut Option<MemoryRows>) {
    let watching = root.watching_statistics();
    #[cfg(target_os = "linux")]
    {
        if watching {
            // Under `--root` it samples only while watched, and writes no file.
            crate::memory::start_watched();
        }
        crate::memory::watch(watching);
    }
    if !watching {
        *memory = None;
        return;
    }
    // Cached by be-host-base for a quarter of a second, so cheap to ask each frame.
    let performance = be_host_base::performance::rows()
        .into_iter()
        .map(statistic_row)
        .collect();
    // Read again only once a new sample came.
    #[cfg(target_os = "linux")]
    {
        let generation = crate::memory::generation();
        if memory
            .as_ref()
            .is_none_or(|shown| shown.generation != generation)
        {
            let view = crate::memory::view();
            // The graph's minutes, which change only with a new sample, so only then.
            let (points, now) = view
                .as_ref()
                .map(|view| {
                    let points = view
                        .points
                        .iter()
                        .map(|point| be_wasm_root_api::MemoryPoint {
                            at: point.at,
                            resident: point.usage.resident,
                            anonymous: point.usage.anonymous,
                            shared: point.usage.shared,
                            wasm: point.usage.wasm,
                        })
                        .collect();
                    (points, view.now)
                })
                .unwrap_or_default();
            root.show_memory_history(points, now);
            let (rows, file) = view
                .map(|view| {
                    let rows = view
                        .rows
                        .into_iter()
                        .map(|memory| statistic_row(memory.row));
                    (rows.collect(), view.file)
                })
                .unwrap_or_default();
            *memory = Some(MemoryRows {
                generation,
                rows,
                file,
            });
        }
    }
    let (rows, file) = memory
        .as_ref()
        .map(|shown| (shown.rows.clone(), shown.file.clone()))
        .unwrap_or_default();
    root.show_statistics(performance, rows, file);
}

/// The memory rows the root was last handed, and the sample they are of.
struct MemoryRows {
    #[cfg_attr(
        not(target_os = "linux"),
        expect(dead_code, reason = "only Linux samples memory")
    )]
    generation: u64,
    rows: Vec<StatisticRow>,
    file: Option<String>,
}

/// One of be-host-base's performance rows, or a memory row, as `be3:root` carries it.
fn statistic_row(row: be_host_base::performance::PerformanceRow) -> StatisticRow {
    StatisticRow {
        heading: row.heading,
        name: row.name,
        current: row.current,
        average: row.average,
        peak: row.peak,
    }
}

/// Answers what the root asked of what `--root` does not run yet (4g7 brings each back), so
/// that each request changes nothing:
/// - The session's screen lock: it runs none (it is refused as a login session), so the lock is
///   unavailable, as `be3:root` answers by default, and asking to lock or a power action from
///   the lock screen changes nothing. `be3:root` drops a password handed over while the session
///   is not locked, so none reaches here.
/// - Displays (the desktop's): it asks to keep none, and an answer changes nothing.
///
/// The block stack it serves (`block_stack`), and its apps' editors (`serve_editors`).
fn refuse_unserved(root: &mut Root) {
    root.take_lock_asked();
    root.take_lock_powers();
    root.take_display_answers();
}

/// Serves the root the editors of the blocks it shows (plans/host_as_platform_adapter.md,
/// 4g7b1): names the app that edits each block it asks of (`host.editor-of`), opens the
/// editor in the app it spawned for it (`host.open-editor`) with be-host-runtime's
/// `Forwarded`, one for each of the root's instances, and serves what each app's editors ask
/// through the root (`host.forward`, `host.forwarded`) as be-host serves its own apps. An
/// instance the root let go of, or that stopped, it stops serving.
fn serve_editors(core: &mut crate::HostCore, root: &mut Root, editors: &mut Editors) {
    let opens = std::mem::take(&mut core.root_opens);
    let core = &*core;
    let served = &mut editors.served;
    let asks: Vec<_> = std::mem::take(&mut editors.asks)
        .into_iter()
        .chain(root.take_editor_asks())
        .collect();
    let openings: Vec<_> = std::mem::take(&mut editors.openings)
        .into_iter()
        .chain(root.take_openings())
        .collect();
    // The stack lists what the root asks for and every block whose editor it serves the root,
    // as it lists the blocks be-host's own editors show: it may not have them yet once it
    // starts on the data it kept, and keeps them while they show.
    let shown_block = |shown: &be_wasm_root_api::Shown| match shown {
        be_wasm_root_api::Shown::Block(shown) => Some(shown.block),
        be_wasm_root_api::Shown::Dialog(_) => None,
    };
    let mut wanted: Vec<[u8; 16]> = asks
        .iter()
        .filter_map(|(_, shown)| shown_block(shown))
        .chain(
            openings
                .iter()
                .filter_map(|opening| shown_block(&opening.shown)),
        )
        .chain(
            editors
                .opened
                .values()
                .filter_map(|opened| opened.block.map(Uuid::into_bytes)),
        )
        // The session's block from the frame the host chose it, before the root asks for its
        // editor: listed only by what is open meanwhile, a change to that would drop it.
        .chain(core.root_session.map(Uuid::into_bytes))
        .collect();
    wanted.sort_unstable();
    wanted.dedup();
    be_host_stack::want(
        be_host_stack::Owner::Root,
        wanted
            .into_iter()
            .map(|block| be_host_stack::Query::Block(Uuid::from_bytes(block)))
            .collect(),
    );
    for (id, shown) in asks {
        // While the stack does not list the session's block (yet, or again for a moment), or
        // has yet to read the listing of the block asked for, an ask for a block it does not
        // list waits for it rather than failing.
        if waits_for_listing(core, &shown) {
            editors.asks.push((id, shown));
            continue;
        }
        let editor = shown_editor(core, &shown).map(|(app, editor, _)| EditorApp {
            app: app.identity.id.clone(),
            module: app.entry_point.clone(),
            editor: editor.into_bytes(),
            name: app.identity.name.clone(),
            preview: app
                .editor(editor.into_bytes())
                .is_some_and(|editor| editor.views.contains(&ParentView::Preview)),
            pan_and_zoom: app
                .editor(editor.into_bytes())
                .is_some_and(|editor| editor.capabilities.pan_and_zoom),
        });
        root.answer_editor(id, editor);
    }
    for opening in openings {
        if waits_for_listing(core, &opening.shown) {
            editors.openings.push(opening);
            continue;
        }
        let opened = open_editor(core, &opening, served, &mut editors.opened);
        root.answer_opening(opening.id, opened);
    }
    for (instance, frames) in root.take_forwarded() {
        let Some(forwarded) = served.get_mut(&instance) else {
            continue;
        };
        match forwarded.receive(frames) {
            Ok(frames) => root.relay(instance, frames),
            Err(why) => {
                // Stopped toward the root too, which shows that it stopped.
                served.remove(&instance);
                root.stop_instance(instance, &format!("it broke the app protocol: {why}"));
            }
        }
    }
    served.retain(|instance, _| root.app_of(*instance).is_some());
    editors
        .opened
        .retain(|(instance, _), _| served.contains_key(instance));
    // What the root's dialogs told the shell, which their placers' statuses say: those of
    // every view the root showed are worded again with it.
    let mut told = false;
    for (instance, editor, messages) in root.take_shell_tells() {
        let Some(forwarded) = served.get_mut(&instance) else {
            continue;
        };
        // A root that encodes what no editor can read has nothing to relay.
        if let Some(messages) = be_sdk_api::ops::decode::<Vec<ShellMessage>>(&messages) {
            forwarded.shell_told(EditorInstanceId(editor), messages);
            told = true;
        }
    }
    let mut views = Vec::new();
    for shown in root.take_shown_children() {
        let placer = (shown.instance, shown.editor);
        editors.placers.retain(|_, held| *held != placer);
        for child in &shown.children {
            if let Some(opened) = &child.shown_in {
                editors
                    .placers
                    .insert((opened.instance, opened.editor), placer);
            }
        }
        let view = (shown.instance, shown.editor, shown.view);
        match shown.children.is_empty() {
            true => editors.shown.remove(&view),
            false => editors.shown.insert(view, shown.children),
        };
        views.push(view);
    }
    editors
        .shown
        .retain(|(instance, ..), _| served.contains_key(instance));
    if told {
        views = editors.shown.keys().copied().collect();
    }
    for (instance, editor, view) in views {
        let statuses = editors
            .shown
            .get(&(instance, editor, view))
            .map(|children| {
                children
                    .iter()
                    .map(|child| child_status(core, &editors.opened, served, child))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(forwarded) = served.get_mut(&instance) {
            forwarded.show_children(EditorInstanceId(editor), ViewId(view), statuses);
        }
    }
    for (instance, editor, event) in root.take_editor_events() {
        let Some(forwarded) = served.get_mut(&instance) else {
            continue;
        };
        // A root that encodes what no editor can read has nothing to tell it.
        if let Some(event) = be_sdk_api::ops::decode::<EditorEvent>(&event) {
            forwarded.tell(EditorInstanceId(editor), event);
        }
    }
    for (instance, editor, updates) in root.take_dock_updates() {
        let Some(forwarded) = served.get_mut(&instance) else {
            continue;
        };
        // A root that encodes what no editor can read has nothing to hand it.
        if let Some(updates) = be_sdk_api::ops::decode::<Vec<be_sdk_api::DockUpdate>>(&updates) {
            forwarded.dock(EditorInstanceId(editor), updates);
        }
    }
    for placed in root.take_placed_contents() {
        place_content(editors, placed);
    }
    for (instance, editor) in root.take_closed_editors() {
        if let Some(forwarded) = editors.served.get_mut(&instance) {
            forwarded.close(EditorInstanceId(editor));
        }
        editors.opened.remove(&(instance, editor));
    }
    for (instance, editor, size) in root.take_assigned_sizes() {
        if let Some(forwarded) = editors.served.get_mut(&instance) {
            forwarded.resized(
                EditorInstanceId(editor),
                beui::vec2(size.width, size.height),
            );
        }
    }
    editors
        .placers
        .retain(|child, _| editors.opened.contains_key(child));
    open_blocks(core, opens, editors);
    for (instance, forwarded) in editors.served.iter_mut() {
        root.relay(*instance, forwarded.step());
    }
}

/// Hands the editor `placed` names, shown as a placed child (`host.place-content`), the part of
/// its world it shows, if it pans and zooms, as be-host's camera does for a placed child
/// (`Camera`'s `shown`): where its content shows, at that rect's scale over the size its content
/// wants.
fn place_content(editors: &mut Editors, placed: be_wasm_root_api::PlacedContent) {
    let pans = editors
        .opened
        .get(&(placed.instance, placed.editor))
        .is_some_and(|opened| {
            opened
                .app
                .editor(opened.editor.into_bytes())
                .is_some_and(|editor| editor.capabilities.pan_and_zoom)
        });
    let Some(forwarded) = editors.served.get_mut(&placed.instance).filter(|_| pans) else {
        return;
    };
    let content = placed.content;
    let rect = beui::Rect::from_min_max(
        beui::pos2(content.min_x, content.min_y),
        beui::pos2(content.max_x, content.max_y),
    );
    let scale = placed
        .intrinsic
        .filter(|intrinsic| intrinsic.width > 0.0 && intrinsic.height > 0.0)
        .map_or(1.0, |intrinsic| {
            (rect.width() / intrinsic.width).min(rect.height() / intrinsic.height)
        });
    forwarded.set_view(
        EditorInstanceId(placed.editor),
        be_host_runtime::EditorView { rect, scale },
    );
}

/// Hands each block the root's editors asked to open (`opens`, by the app and instance that
/// asked) to the editor that handles show requests, as be-host's `show_block` does: the nearest
/// of the asking editor and those that place it whose manifest accepts them, else the editor
/// showing the session's block, which be-host's own shell is. An ask from an editor the root
/// no longer shows goes to the editor showing the session's block, as be-host hands one from
/// no editor it knows to its shell; with no session shown, it is dropped.
fn open_blocks(
    core: &crate::HostCore,
    opens: Vec<(String, EditorInstanceId, be_host_runtime::BlockOpen)>,
    editors: &mut Editors,
) {
    for (app, instance, open) in opens {
        let from = editors
            .opened
            .iter()
            .find(|((_, editor), opened)| *editor == instance.0 && opened.app.identity.id == app)
            .map(|(key, _)| *key);
        let shell = editors
            .opened
            .iter()
            .find(|(key, opened)| {
                opened
                    .block
                    .is_some_and(|block| Some(block) == core.root_session)
                    && !editors.placers.contains_key(key)
            })
            .map(|(key, _)| *key);
        let Some(handler) = handler(from, shell, editors) else {
            continue;
        };
        if let Some(forwarded) = editors.served.get_mut(&handler.0) {
            forwarded.show_block(EditorInstanceId(handler.1), open);
        }
    }
}

/// The editor that handles a show request from `from` (be-host's `shell_route::handler`): the
/// nearest of it and the editors placing it, up to the `shell`, whose manifest accepts show
/// requests, else the shell.
fn handler(
    from: Option<(u64, u64)>,
    shell: Option<(u64, u64)>,
    editors: &Editors,
) -> Option<(u64, u64)> {
    let accepts = |key: &(u64, u64)| {
        editors.opened.get(key).is_some_and(|opened| {
            opened
                .app
                .editor(opened.editor.into_bytes())
                .is_some_and(|editor| editor.accepts(be_sdk_manifest::ShellRequest::ShowBlock))
        })
    };
    let mut visited = Vec::new();
    let mut current = from;
    while let Some(key) = current {
        if Some(key) == shell || visited.contains(&key) {
            break;
        }
        if accepts(&key) {
            return Some(key);
        }
        visited.push(key);
        current = editors.placers.get(&key).copied();
    }
    shell
}

/// Whether an ask for an editor of `block` waits: the stack does not list it, but may yet, while
/// the session's block is not listed or the stack has yet to read the block's listing, which
/// `serve_editors` asks it for.
/// Whether an ask naming `shown` waits for the stack to list its block (`unlisted_yet`): a
/// dialog names none.
fn waits_for_listing(core: &crate::HostCore, shown: &be_wasm_root_api::Shown) -> bool {
    match shown {
        be_wasm_root_api::Shown::Block(shown) => unlisted_yet(core, shown.block),
        be_wasm_root_api::Shown::Dialog(_) => false,
    }
}

/// What a child the root opens an editor for shows, as the host opens it (`shown_editor`).
enum Shows {
    Block(crate::editors::Opened),
    Picker(be_host_runtime::PickerRequest),
    Rename(Uuid),
    Export(EditorBlock, be_sdk_api::ExportRequest),
}

/// The app and editor that run what the root's child shows, and how it is opened: a block's
/// editor (`HostCore::editor_of`), or the editor that runs a dialog an editor placed, as
/// be-host's registry chooses it (`picker`, `rename`, `export`).
fn shown_editor(
    core: &crate::HostCore,
    shown: &be_wasm_root_api::Shown,
) -> Result<(std::sync::Arc<be_sdk_manifest::AppManifest>, Uuid, Shows), String> {
    let content = match shown {
        be_wasm_root_api::Shown::Block(shown) => {
            return core
                .editor_of(shown)
                .map(|(app, editor, opened)| (app, editor, Shows::Block(opened)));
        }
        be_wasm_root_api::Shown::Dialog(content) => {
            be_sdk_api::ops::decode::<be_sdk_api::ChildContent>(content)
                .ok_or("the root named a dialog the host cannot read")?
        }
    };
    match content {
        be_sdk_api::ChildContent::Picker { filter, parent } => {
            let (app, editor) = core
                .registry
                .accepting(be_sdk_manifest::ShellRequest::Pick)
                .ok_or("no app picks blocks")?;
            let request = be_host_runtime::PickerRequest {
                filter,
                parent,
                sheet: false,
            };
            Ok((app, editor, Shows::Picker(request)))
        }
        be_sdk_api::ChildContent::Rename(block) => {
            let (app, editor) = core
                .registry
                .accepting(be_sdk_manifest::ShellRequest::Rename)
                .ok_or("no app renames blocks")?;
            Ok((app, editor, Shows::Rename(Uuid::from_bytes(block))))
        }
        be_sdk_api::ChildContent::Export { block_id, request } => {
            let block = Uuid::from_bytes(block_id);
            let node = be_host_stack::node(block).ok_or("no block of that id is listed")?;
            let (app, editor) = core
                .registry
                .exporting(node.content_type, &request.exporter)
                .ok_or("the block's editor has no such exporter")?;
            let opened = EditorBlock {
                id: block,
                block_type: node.content_type,
                view_block: None,
            };
            Ok((app, editor, Shows::Export(opened, request)))
        }
        _ => Err("the host runs no dialog of that kind".to_owned()),
    }
}

fn unlisted_yet(core: &crate::HostCore, block: [u8; 16]) -> bool {
    let block = Uuid::from_bytes(block);
    let query = be_host_stack::Query::Block(block);
    let reading = !be_host_stack::ready(query) && be_host_stack::failure(query).is_none();
    (core.root_session_pending || reading) && be_host_stack::node(block).is_none()
}

/// The editors be-host serves the root's apps, by the root's instance each runs in, and the
/// root's asks for editors of blocks the stack does not list yet, which wait until it does.
#[derive(Default)]
struct Editors {
    served: HashMap<u64, Forwarded>,
    /// Each editor opened, by the root's instance and the editor: what its app's manifest says
    /// of it, for the statuses of the children it shows.
    opened: HashMap<(u64, u64), OpenedFor>,
    asks: Vec<(u64, be_wasm_root_api::Shown)>,
    openings: Vec<EditorOpening>,
    /// The children each view of the editors the root shows placed, as the root last said
    /// (`host.show-children`), by the root's instance, the editor and the view: what their
    /// statuses are worded from again once a dialog among them told the shell something.
    shown: HashMap<(u64, u64, u64), Vec<be_wasm_root_api::ShownChild>>,
    /// The editor placing each editor the root shows as a child, as the root last said
    /// (`host.show-children`), each by the root's instance and the editor: what a show request
    /// climbs to find its handler.
    placers: HashMap<(u64, u64), (u64, u64)>,
}

/// An editor be-host opened in an app the root spawned: the app's manifest, the editor it opened
/// and its block, none for a dialog (a picker, a rename form, an Export dialog).
struct OpenedFor {
    app: std::sync::Arc<be_sdk_manifest::AppManifest>,
    editor: Uuid,
    block: Option<Uuid>,
}

/// Said of a child the root shows nothing of and gave no reason for: one whose editor is still
/// being opened.
const CHILD_OPENING: &str = "the block's editor is opening";

/// The status of a child an editor the root embeds placed, as the root shows it
/// (`host.show-children`): available once an editor shows it, with what its app's manifest
/// says of that editor (be-host's `status_of`, for its own children). A preview is available
/// at once, as under be-host (`preview_of`): waiting while its editor opens, then shown in its
/// place by the root, or drawn as its block's label by its placer when its editor draws none.
fn child_status(
    core: &crate::HostCore,
    opened: &HashMap<(u64, u64), OpenedFor>,
    served: &HashMap<u64, Forwarded>,
    child: &be_wasm_root_api::ShownChild,
) -> HostChildStatus {
    let shown = child
        .shown_in
        .as_ref()
        .and_then(|shown| opened.get(&(shown.instance, shown.editor)));
    let manifest = shown.and_then(|shown| shown.app.editor(shown.editor.into_bytes()));
    // Until its editor is open, the editor that will show it says how it is placed (its
    // interaction, capabilities and resizing), as be-host knows it at once.
    let edits = child
        .shows
        .as_ref()
        .filter(|_| manifest.is_none())
        .and_then(|shows| shown_editor(core, shows).ok());
    let described = manifest.or_else(|| {
        edits
            .as_ref()
            .and_then(|(app, editor, _)| app.editor(editor.into_bytes()))
    });
    let preview = match (child.preview, manifest) {
        (ShownPreview::None, _) => ChildPreview::Hosted,
        (ShownPreview::Label, _) => ChildPreview::Label,
        (ShownPreview::View, Some(_)) => ChildPreview::Composited,
        (ShownPreview::View, None) => ChildPreview::Waiting,
    };
    let available =
        manifest.is_some() || (child.preview != ShownPreview::None && child.error.is_none());
    // A dialog's pick, answer and card, as what it told the shell that the root relays says
    // (`host.tell-shell`), as be-host's `picker_status` says them.
    let dialog = child
        .shown_in
        .as_ref()
        .filter(|_| shown.is_some_and(|shown| shown.block.is_none()))
        .and_then(|shown| {
            let forwarded = served.get(&shown.instance)?;
            let editor = EditorInstanceId(shown.editor);
            Some((
                forwarded.picked(editor),
                forwarded.dialog_status(editor),
                forwarded.card(editor),
            ))
        });
    let error = shown
        .and_then(|shown| shown.block)
        .and_then(be_host_stack::revisions::error)
        .or_else(|| child.error.clone())
        .or_else(|| (!available).then(|| CHILD_OPENING.to_owned()));
    HostChildStatus {
        child: ChildId(child.child),
        available,
        intrinsic: child
            .intrinsic
            .as_ref()
            .map(|size| beui::vec2(size.width, size.height)),
        active: available && child.active,
        interaction: described.map_or(InteractionMode::Preview, |editor| editor.interaction),
        capabilities: described
            .map_or_else(EditorCapabilities::default, |editor| editor.capabilities),
        resize: described.map_or(ResizeMode::None, |editor| editor.resize),
        error,
        // A root that encodes what no editor can read shows the child with no menu.
        menu: be_sdk_api::ops::decode::<Vec<be_sdk_api::MenuItem>>(&child.menu)
            .and_then(|items| be_sdk_api::MenuEntry::nest(items).ok())
            .unwrap_or_default(),
        creation: None,
        preview,
        dialog: dialog
            .as_ref()
            .map_or_else(DialogStatus::default, |(_, dialog, _)| *dialog),
        picked: dialog.as_ref().and_then(|(picked, ..)| picked.clone()),
        card: dialog.as_ref().is_some_and(|(.., card)| *card),
        title: None,
        icon: None,
        group: None,
        raised: 0,
        rung: 0,
        minimized: 0,
        fullscreen: false,
        notice: None,
        menu_asked: None,
    }
}

/// Opens the editor `opening` asks for in the app the root's instance runs, which must be the
/// one that edits the block.
fn open_editor(
    core: &crate::HostCore,
    opening: &EditorOpening,
    served: &mut HashMap<u64, Forwarded>,
    opened_for: &mut HashMap<(u64, u64), OpenedFor>,
) -> Result<OpenedEditor, String> {
    let (app, editor, shows) = shown_editor(core, &opening.shown)?;
    if opening.app.as_deref() != Some(app.identity.id.as_str()) {
        return Err(format!(
            "{} runs it, which the root's instance does not run",
            app.identity.id
        ));
    }
    let instance = crate::editors::app::next_instance();
    let (role, block) = match shows {
        Shows::Block(opened) => (
            InstanceRole::Editor(
                editor,
                EditorBlock {
                    id: opened.id,
                    block_type: opened.block_type,
                    view_block: opened.view_block,
                },
            ),
            Some(opened.id),
        ),
        Shows::Picker(request) => {
            be_host_runtime::ask_picker(instance, request);
            (InstanceRole::Picker(editor), None)
        }
        Shows::Rename(block) => (InstanceRole::Rename(editor, block, false), None),
        Shows::Export(block, request) => {
            be_host_runtime::ask_export(instance, request);
            (InstanceRole::Export(editor, block), None)
        }
    };
    opened_for.insert(
        (opening.instance, instance.0),
        OpenedFor {
            app: std::sync::Arc::clone(&app),
            editor,
            block,
        },
    );
    let frames = served
        .entry(opening.instance)
        .or_insert_with(|| Forwarded::new(&app))
        .open(
            instance,
            role,
            core.client_id,
            core.registry.app_block_types(),
        );
    Ok(OpenedEditor {
        editor: instance.0,
        frames,
    })
}

/// The host's notices as `be3:root` carries them, oldest first.
fn shown_notices() -> Vec<Notice> {
    notices::shown()
        .into_iter()
        .map(|notice| Notice {
            id: notice.id,
            failure: notice.failure(),
            actions: notice
                .actions()
                .into_iter()
                .map(|(key, label)| NoticeAction {
                    key: key.to_owned(),
                    label: label.to_owned(),
                })
                .collect(),
            message: notice.message,
        })
        .collect()
}

/// Does what the person answered the host's notices with through the root, in order: dismisses
/// a notice answered with no action, copies its report, or opens its link and dismisses it.
fn answer_notices(context: &beui::native::Context, answers: Vec<(u64, Option<String>)>) {
    for (id, action) in answers {
        match action.as_deref() {
            // Under `--root` no account is signed in, so no notice is an account's to dismiss
            // on the server too.
            None => {
                notices::dismiss(id);
            }
            Some(notices::COPY_REPORT) => {
                if let Some(report) = notices::report_of(id) {
                    context.copy(Clip::text(report));
                }
            }
            Some(notices::OPEN_LINK) => {
                if let Some(link) = notices::link_of(id)
                    && let Err(error) = be_host_base::platform::open_link(&link)
                {
                    notices::report(error);
                }
                notices::dismiss(id);
            }
            // A key the notice does not offer changes nothing.
            Some(_) => {}
        }
    }
}

/// Puts what `root` copied on the window's clipboard, the last copy staying, and reads the
/// clipboard for each paste it asked for, whose answer `pasted` keeps until the next frame hands
/// it over (`Root::answer_paste`). A copied item that cannot be read is left out, and a copy
/// with none that can be read changes nothing.
fn clipboard(
    context: &beui::native::Context,
    root: &mut Root,
    pasted: &Rc<RefCell<Vec<(u64, Option<ClipItem>)>>>,
) {
    for items in root.take_copies() {
        let items: Vec<ClipItem> = items
            .iter()
            .filter_map(|item| be_sdk_api::ops::decode::<ClipItem>(item))
            .collect();
        // A copy none of whose items can be read leaves the clipboard as it was.
        if !items.is_empty() {
            context.copy(Clip::items(items));
        }
    }
    for (id, formats) in root.take_pastes() {
        let pasted = Rc::clone(pasted);
        context.read_clipboard(formats, move |item| pasted.borrow_mut().push((id, item)));
    }
}

/// The fonts be-host bundles, which it tells its apps, as `be3:root` hands them to its root.
fn fonts() -> Vec<u8> {
    be_sdk_api::ops::encode(&be_host_runtime::bundled_fonts())
}

fn nanos(time: Duration) -> u64 {
    u64::try_from(time.as_nanos()).unwrap_or(u64::MAX)
}

/// The surfaces of the root's frame the window shows, bottom first, each with the texture it
/// names.
struct ShownLayers {
    layers: Vec<(Layer, wgpu::Texture)>,
    quads: Quads,
    /// What the root's own surfaces painted when drive last asked, by their ids.
    paintings: Rc<RefCell<BTreeMap<u64, Painting>>>,
}

impl Paints for ShownLayers {
    /// Each of the root's surfaces' paintings where its layer shows it in the window, bottom
    /// first: its own by their surfaces' ids, and its apps' screens, which their paintings
    /// come through the root for (plans/host_as_platform_adapter.md, Other moves), by their
    /// layers' ids.
    fn painting(&self, _rect: beui::Rect) -> Option<Painting> {
        let paintings = self.paintings.borrow();
        let mut primitives = Vec::new();
        let mut textures = BTreeMap::new();
        for (layer, _) in &self.layers {
            let key = match layer.surface {
                Surface::Root(surface) => u64::from(surface),
                Surface::Screen { .. } => layer.id.0,
                Surface::Anchored { .. } | Surface::Host(_) => continue,
            };
            let Some(painting) = paintings.get(&key) else {
                continue;
            };
            let (width, height) = (layer.size.0 as f32, layer.size.1 as f32);
            let corners =
                [(0.0, 0.0), (width, 0.0), (width, height), (0.0, height)].map(|(x, y)| {
                    let at = layer.transform.apply(beui::pos2(x, y));
                    [at.x, at.y]
                });
            let clip = [
                corners.iter().map(|at| at[0]).fold(f32::MAX, f32::min),
                corners.iter().map(|at| at[1]).fold(f32::MAX, f32::min),
                corners.iter().map(|at| at[0]).fold(f32::MIN, f32::max),
                corners.iter().map(|at| at[1]).fold(f32::MIN, f32::max),
            ];
            primitives.extend(painting.placed(Placement {
                source: [0.0, 0.0, 1.0, 1.0],
                corners,
                clip,
                corner_radius: layer.corner_radius,
            }));
            for (key, pixels) in &painting.textures {
                textures.entry(*key).or_insert_with(|| pixels.clone());
            }
        }
        Some(Painting {
            frame: beui::native::painting::Frame {
                size: [1, 1],
                pixels_per_point: 1.0,
                background: [0; 4],
                primitives,
            },
            textures,
        })
    }
}

impl Draw for ShownLayers {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        at: DrawAt,
    ) {
        let pipelines = Pipelines::of(device, &at);
        let bound: Vec<_> = self
            .layers
            .iter()
            .map(|(layer, texture)| {
                let quad = Quad {
                    clip: at.clip,
                    // An app presents sRGB-encoded values, as it does to the host's own UI.
                    srgb_encoded: true,
                    ..compose::quad(layer, at.pixels_per_point, [0.0; 2])
                };
                (pipelines.bind(texture), quad)
            })
            .collect();
        self.quads.prepare(
            &pipelines,
            queue,
            at.format,
            [at.screen.x, at.screen.y],
            bound.iter().map(|(texture, quad)| (texture, *quad)),
        );
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {
        self.quads.paint(pass);
    }
}
