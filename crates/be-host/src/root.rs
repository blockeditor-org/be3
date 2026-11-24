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
//! something to step for (`Root::on_wake`). `drive paint` paints what the root's own surfaces
//! painted (`be3:root`'s `paintings`) where its layer list shows them; an app's screen it
//! embeds is not in the painting yet.
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
//! It runs none of the host's core: no data directory or the lock that keeps a second be-host
//! off it, no block stack, workspace or accounts, no screen lock, and none of the desktop's OS
//! services. Step 4g brings back under this program what the host keeps (the plan's What the
//! host keeps) before it becomes the default; until then `--root` is refused as a login session
//! (`BEUI_PLATFORM=drm`), which would have no lock screen.
//!
//! The root is the app: one that fails to start or stops closes the window, and be-host exits
//! saying why, as it would on a panic.

use std::{
    cell::RefCell,
    collections::BTreeMap,
    error::Error,
    rc::Rc,
    time::{Duration, Instant},
};

use be_host_base::notices;
use be_lib_layers::{Layer, LayerList, Surface};
use be_lib_ui_types::ClipItem;
use be_lib_ui_types::input::Timed;
use be_sdk_app_host::pacing::FrameClock;
use be_wasm_root_api::{FrameRequest, Notice, NoticeAction, StatisticRow, WindowSize};
use be_wasm_root_host::Root;
use beui::native::painting::{Painting, Paints, Placement, ScreenPaintings};
use beui::native::texture_quad::{Pipelines, Quad, Quads};
use beui::native::{Draw, DrawAt, Drawing as GpuDrawing, compose, draw_gpu};
use beui::reactive::*;
use beui::styled::*;
use beui::{CursorIcon, Input, WindowId};

/// The id what the root shows has in the accessibility tree, `#root` to drive.
pub(crate) const SHOWN: &str = "root";

/// Runs `app`'s entry point as the root in a window opened with `options`, until the window
/// closes or the root stops, which is an error.
pub(crate) async fn run(
    app: String,
    options: beui_host::native::RunOptions,
) -> Result<(), Box<dyn Error>> {
    let stopped: Rc<RefCell<Option<String>>> = Rc::default();
    beui_host_window::run_program(options, RootShell::new(app, Rc::clone(&stopped))).await?;
    match stopped.take() {
        Some(why) => Err(why.into()),
        None => Ok(()),
    }
}

struct RootShell {
    app: String,
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
    paintings: Rc<RefCell<BTreeMap<u32, Painting>>>,
    /// Whether drive asked for a painting the root has not answered yet: be-host is busy until
    /// it has.
    painting: bool,
    /// Wakes the window for a frame, which asks the root for what drive asked.
    waker: Option<beui::native::Waker>,
    /// The generation of the host's notices the root was last handed.
    notices: Option<u64>,
    /// What the window's clipboard answered the root's pastes with, by their ids, until the
    /// root is handed them.
    pasted: Rc<RefCell<Vec<(u64, Option<ClipItem>)>>>,
    /// The memory rows the root was last handed while it watches the host's statistics.
    memory: Option<MemoryRows>,
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
    fn new(app: String, stopped: Rc<RefCell<Option<String>>>) -> Self {
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
            pasted: Rc::default(),
            memory: None,
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
        refuse_unserved(root);
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
            *self.paintings.borrow_mut() = painted
                .screens
                .into_iter()
                .filter_map(|(id, painting)| Some((u32::try_from(id).ok()?, painting)))
                .collect();
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
        let mut busy = Vec::new();
        if self.owed {
            busy.push(format!("the root {} owes a frame", self.app));
        }
        if self.painting && self.root.is_some() {
            busy.push(format!("the root {} owes a painting", self.app));
        }
        busy
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
/// - The block stack: it shows nothing and waits for nothing, so a command changes nothing,
///   and a secret asked for is answered with none. It opens no block's editor either
///   (`host.editor-of` and `host.open-editor` say why), so no app the root spawns has an editor
///   whose calls the root forwards; be-host-runtime's `Forwarded` serves them once the stack
///   runs here (plans/host_as_platform_adapter.md, 4g7b2).
fn refuse_unserved(root: &mut Root) {
    root.take_lock_asked();
    root.take_lock_powers();
    root.take_display_answers();
    root.take_core_commands();
    for (id, _) in root.take_secret_requests() {
        root.answer_secret(id, None);
    }
    for (id, _) in root.take_editor_asks() {
        root.answer_editor(id, Err(NO_BLOCK_STACK.to_owned()));
    }
    for opening in root.take_openings() {
        root.answer_opening(opening.id, Err(NO_BLOCK_STACK.to_owned()));
    }
    root.take_forwarded();
}

/// Why be-host opens no block's editor for its root yet.
const NO_BLOCK_STACK: &str = "be-host runs no block stack under --root yet";

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
    paintings: Rc<RefCell<BTreeMap<u32, Painting>>>,
}

impl Paints for ShownLayers {
    /// Each of the root's own surfaces' paintings where its layer shows it in the window, bottom
    /// first. An app's screen the root embeds is left out: its painting would come
    /// through the root (plans/host_as_platform_adapter.md, Other moves).
    fn painting(&self, _rect: beui::Rect) -> Option<Painting> {
        let paintings = self.paintings.borrow();
        let mut primitives = Vec::new();
        let mut textures = BTreeMap::new();
        for (layer, _) in &self.layers {
            let Surface::Root(surface) = layer.surface else {
                continue;
            };
            let Some(painting) = paintings.get(&surface) else {
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
