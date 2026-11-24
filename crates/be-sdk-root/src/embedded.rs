//! One app a root guest spawned, run as its host: the app protocol's host half over the
//! instance's channel.

use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::pin,
    rc::{Rc, Weak},
    task::{Poll, Waker},
    time::Duration,
};

use be_lib_geometry::{Pos2, Rect, pos2, vec2};
use be_lib_layers::{Affine, Layer, LayerId, LayerInput, Surface};
use be_lib_stable_hash::HashMap;
use be_lib_ui_types::input::{Input, PenPhase, TouchPhase};
use be_sdk_api::{
    ChildPlacement, ChildPlacements, DockMessage, EditorInstanceId, Fonts, ImeArea, InputBatch,
    Insets, Opening, Preferences, ScreenId, ScreenPlacement, ScreenRequest, SurfaceSpec,
    TimedInput, WindowRequests, embed::shell::Call as ShellCall,
};
use be_sdk_app_host::{
    calls::{AppCall, Heard, Outbound},
    holds::Held,
    pacing::{Ask, Asking, FRAME_TIMEOUT, Pacing},
    run::{Run, Telling},
    session::{Session, SessionFailure, SessionState},
    wire::Wire,
};
use be_wasm_root_api::{ShownBlock, guest::host};
use be_wasm_sandbox_api::{
    guest::sandbox::Instance,
    sandbox::{Config, Source, StagedModule},
};

/// The host's name an embedded app hears with its hello.
const HOST_NAME: &str = "be3 root";

/// The most memory an app's instance may hold: an app links its libraries into one shared
/// memory, which the base library alone fills much of.
const APP_MEMORY: u64 = 1 << 30;

/// What `Embedded::spawn` starts: a staged app's entry point, the editor it opens and the
/// screens it asks of it.
pub struct Spawn {
    /// The staged app, by the id its manifest gives it.
    pub app: String,
    /// Its entry point, as its manifest names it.
    pub module: String,
    pub opening: Opening,
    pub screens: Vec<ScreenRequest>,
    /// The surface the app draws to, granted if its hello asks for one.
    pub surface: SurfaceSpec,
    /// The fonts the app is told once it runs, which it draws its text in: those its root's
    /// host bundles (`be3:root`'s `host.fonts`), shared by every app the root spawns.
    pub fonts: Rc<Fonts>,
}

/// What `Embedded::spawn_block` starts: the editor of a block, which the root's host opens and
/// serves (`be3:root`'s `host.open-editor`), in the app it names (`host.editor-of`).
pub struct BlockSpawn {
    /// The block, and the editor and view block its placement names, if any.
    pub shown: ShownBlock,
    /// The screens asked of the editor, given the instance the host opened it as.
    pub screens: Box<dyn FnOnce(EditorInstanceId) -> Vec<ScreenRequest>>,
    /// The surface the app draws to, granted if its hello asks for one.
    pub surface: SurfaceSpec,
    /// The fonts the app is told once it runs (`Spawn::fonts`).
    pub fonts: Rc<Fonts>,
}

/// A block's editor its root's host opened, in the app `Embedded::spawn_block` spawned.
pub struct Hosted {
    pub app: Embedded,
    /// The editor's instance, which its screens name.
    pub editor: EditorInstanceId,
    /// The app's name, in words.
    pub name: String,
}

/// Why `Embedded::spawn_block` spawned nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnFailure {
    /// The name of the app that edits the block, once the host named it.
    pub name: Option<String>,
    pub why: String,
}

/// The editor an app opens once it says hello.
enum Opened {
    /// One the root opened itself, whose host serves it nothing: what it asks of a host is
    /// refused.
    Root(Opening),
    /// One the root's host opened, by the frames that open it, which the root relays as they
    /// are; everything the app then sends under it the root forwards to its host
    /// (`host.forward`), and what the host answers it relays (`host.forwarded`).
    Host(Vec<Vec<u8>>),
}

/// An app the root spawned and runs as its host. Clones are the same app. It stops when the
/// last clone goes (`Embedding::remove` lets go of the embedding's), or with the root.
#[derive(Clone)]
pub struct Embedded {
    state: Rc<RefCell<State>>,
    /// The instance's `instance` resource's op: the handle the root and its host both know it by.
    instance: u64,
}

struct State {
    instance: Rc<Instance>,
    wire: Wire,
    /// The app's run: its session, pacing, holds and layout. The root holds no answers for a
    /// later poll, so its holds keep only layouts.
    run: Run<()>,
    /// The fonts it is told once it runs.
    fonts: Rc<Fonts>,
    /// The frames that open the editor its root's host opened, relayed once the app says hello.
    opening: Vec<Vec<u8>>,
    /// Whether the app said hello: one that stops after it stopped, one that stops before it
    /// could not start.
    started: bool,
    /// What the app answered it painted (`Embedded::paintings`), until taken.
    painted_answer: Option<Vec<u8>>,
    /// The frame clock's time, as the last frame was asked for.
    now: Duration,
    /// The root's frames so far, the pass the app's pacing counts.
    pass: u64,
    /// What each screen showed when the app last painted it.
    painted: HashMap<ScreenId, ScreenPlacement>,
    /// The screen each surface showed when last painted, kept once the screen goes, so the
    /// release of a press on it still reaches the app.
    surfaces: HashMap<u32, ScreenPlacement>,
    /// What each screen asks of its window (its input method's area, its cursor), as its
    /// last layout applied said.
    windows: HashMap<ScreenId, WindowRequests>,
    /// The children each of its editors' views placed, as its last layout applied said
    /// (`Embedded::placed`).
    placed: Vec<ChildPlacements>,
    /// Whether what its views placed changed since the root last read it (`Embedded::placed`):
    /// the root owes its host the frame that shows that.
    placing: bool,
    /// What its docks said since the root last read it (`Embedded::take_docks`), in order,
    /// each applied once the children of the commit it came in are.
    docks: Vec<DockMessage>,
    /// Where each screen holds its editor's content, as its last layout's frame report said:
    /// none for a view that holds none (a dock bar of its tools).
    contents: HashMap<ScreenId, Option<Insets>>,
    /// Why the app stopped, once it did: it is asked for nothing again.
    failure: Option<String>,
    /// The frames waiting for the app to say hello or present.
    waiting: Vec<Waker>,
    /// Stops the task hearing the app once the state goes.
    stop: Rc<Stop>,
}

/// Tells the task hearing an app that its last `Embedded` went, so it lets go of the instance,
/// which stops it, even while the app sends nothing.
#[derive(Default)]
struct Stop {
    stopped: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

impl Drop for State {
    fn drop(&mut self) {
        self.stop.stopped.set(true);
        if let Some(waker) = self.stop.waker.take() {
            waker.wake();
        }
    }
}

impl Embedded {
    /// Spawns `spawn`'s app at the frame clock's `now` and opens its editor once it says hello.
    /// Call it on the root's executor, from a frame or an input: the app is heard by a task of
    /// its own there.
    pub fn spawn(spawn: Spawn, now: Duration) -> Self {
        let instance = instance(spawn.app, spawn.module);
        Self::start(
            instance,
            Opened::Root(spawn.opening),
            spawn.screens,
            spawn.surface,
            spawn.fonts,
            now,
        )
    }

    /// Spawns the app that edits `spawn`'s block, as its root's host names it
    /// (`host.editor-of`), and has the host open the block's editor in it (`host.open-editor`),
    /// which it serves from then on. Call it on the root's executor at the frame clock's `now`;
    /// it answers once the host did, or with why the host would not.
    pub async fn spawn_block(spawn: BlockSpawn, now: Duration) -> Result<Hosted, SpawnFailure> {
        let editor = host::editor_of(spawn.shown.clone())
            .await
            .map_err(|error| {
                format!("the host did not answer which app edits the block: {error:?}")
            })
            .and_then(|editor| editor)
            .map_err(|why| SpawnFailure { name: None, why })?;
        let name = editor.name;
        let failed = |why: String| SpawnFailure {
            name: Some(name.clone()),
            why,
        };
        let instance = instance(editor.app, editor.module);
        let opened = host::open_editor(instance.op(), spawn.shown)
            .await
            .map_err(|error| format!("the host did not open the block's editor: {error:?}"))
            .and_then(|opened| opened)
            .map_err(failed)?;
        let editor = EditorInstanceId(opened.editor);
        let screens = (spawn.screens)(editor);
        let app = Self::start(
            instance,
            Opened::Host(opened.frames),
            screens,
            spawn.surface,
            spawn.fonts,
            now,
        );
        Ok(Hosted { app, editor, name })
    }

    /// Asks the app for `screens` in place of those it was asked for, as when the window it
    /// shows in is resized: at once once it runs, else with the rest it is sent once it says
    /// hello.
    pub fn show(&self, screens: Vec<ScreenRequest>) {
        let mut state = self.state.borrow_mut();
        if state.failure.is_some() || state.run.screens == screens {
            return;
        }
        state.run.screens.clone_from(&screens);
        let send = state.run.queue([AppCall::Screens(screens).into()]);
        state.run.pacing.need();
        state.wire.send(send);
        state.flush();
    }

    /// What the app paints its screens with now, as it answers a host's `paintings` for its
    /// tests (be-sdk-api's `ScreenPaintings`, encoded): none for an app that does not run.
    pub async fn paintings(&self) -> Option<Vec<u8>> {
        {
            let mut state = self.state.borrow_mut();
            if state.failure.is_some() || !state.run.session.running() {
                return None;
            }
            state.painted_answer = None;
            state.wire.app_call(AppCall::Paintings);
            state.flush();
        }
        std::future::poll_fn(|context| {
            let mut state = self.state.borrow_mut();
            if let Some(painted) = state.painted_answer.take() {
                return Poll::Ready(Some(painted));
            }
            if state.failure.is_some() {
                return Poll::Ready(None);
            }
            if !state
                .waiting
                .iter()
                .any(|waker| waker.will_wake(context.waker()))
            {
                state.waiting.push(context.waker().clone());
            }
            Poll::Pending
        })
        .await
    }

    /// Whether the app said hello: one that stops after it stopped, one that stops before it
    /// could not start.
    pub fn started(&self) -> bool {
        self.state.borrow().started
    }

    fn start(
        instance: Rc<Instance>,
        opened: Opened,
        screens: Vec<ScreenRequest>,
        surface: SurfaceSpec,
        fonts: Rc<Fonts>,
        now: Duration,
    ) -> Self {
        let mut run = Run::new(
            Session::new(HOST_NAME, Some(surface)),
            false,
            Preferences::default(),
        );
        run.session.start(millis(now));
        run.screens.clone_from(&screens);
        let (open, opening) = match opened {
            Opened::Root(opening) => (Some(Outbound::Open(Box::new(opening))), Vec::new()),
            Opened::Host(frames) => (None, frames),
        };
        // Held until the hello: the session does not run yet.
        let _ = run.queue(open.into_iter().chain([AppCall::Screens(screens).into()]));
        run.pacing.need();
        let handle = instance.op();
        let stop = Rc::new(Stop::default());
        let state = Rc::new(RefCell::new(State {
            instance: Rc::clone(&instance),
            wire: Wire::default(),
            run,
            fonts,
            opening,
            started: false,
            painted_answer: None,
            now,
            pass: 0,
            painted: HashMap::default(),
            surfaces: HashMap::default(),
            windows: HashMap::default(),
            placed: Vec::new(),
            placing: false,
            docks: Vec::new(),
            contents: HashMap::default(),
            failure: None,
            waiting: Vec::new(),
            stop: Rc::clone(&stop),
        }));
        if !state.borrow().opening.is_empty() {
            relay_to(handle, &state);
        }
        be_wasm_ops::guest::spawn(hear(Rc::downgrade(&state), instance, stop));
        Self {
            state,
            instance: handle,
        }
    }

    /// The id of the layer that shows its screen `screen` (`layer`), painted or not.
    pub fn layer_id(&self, screen: ScreenId) -> LayerId {
        screen_layer(self.instance, screen)
    }

    /// The children its editors' views placed, each view's as its last layout applied said:
    /// what a root shows of them is its own (`Children`).
    pub fn placed(&self) -> Vec<ChildPlacements> {
        let mut state = self.state.borrow_mut();
        state.placing = false;
        state.placed.clone()
    }

    /// What its editors' docks said since the last call, in order (be-sdk-api's `DockMessage`):
    /// what a root's docks hear of the app (`Children`), once it took in what it placed with
    /// them.
    pub fn take_docks(&self) -> Vec<DockMessage> {
        std::mem::take(&mut self.state.borrow_mut().docks)
    }

    /// Where its screen `screen` holds its editor's content, in the screen's points, as its last
    /// layout said: none for a view that holds none, or one not laid out yet.
    pub fn content(&self, screen: ScreenId) -> Option<Insets> {
        self.state.borrow().contents.get(&screen).copied().flatten()
    }

    /// The screens it was asked for last.
    pub fn screens(&self) -> Vec<ScreenRequest> {
        self.state.borrow().run.screens.clone()
    }

    /// The instance's handle, which a layer naming one of its surfaces carries
    /// (`Surface::Screen`'s `instance`) and the root's host resolves.
    pub fn instance(&self) -> u64 {
        self.instance
    }

    /// Why the app stopped, if it did.
    pub fn failure(&self) -> Option<String> {
        self.state.borrow().failure.clone()
    }

    /// Asks the app for a frame for the frame clock's `now`, if its pacing says one is due, and
    /// answers once it presented, or at once when none is due. A frame asked for earlier and not
    /// yet presented is waited for instead. Before the app says hello it waits for that first.
    /// The root races it against its deadline: dropped, the frame asked for is still taken in
    /// when the app presents it.
    pub async fn frame(&self, now: Duration) {
        {
            let mut state = self.state.borrow_mut();
            state.now = now;
            state.pass += 1;
            state.run.session.tick(millis(now));
            state.failed_session();
        }
        // Asked once the session runs, and again while the app has yet to take in its input.
        let mut asked = None;
        std::future::poll_fn(|context| {
            let mut state = self.state.borrow_mut();
            if state.failure.is_some() {
                return Poll::Ready(());
            }
            if state.run.session.running() {
                if matches!(asked, None | Some(Ask::AfterInput)) {
                    asked = Some(state.ask());
                }
                // The pacing says whether a frame is in flight, this one's or an earlier one's
                // (asked again past `FRAME_TIMEOUT`).
                if asked != Some(Ask::AfterInput) && !state.run.pacing.in_flight() {
                    return Poll::Ready(());
                }
            }
            if !state
                .waiting
                .iter()
                .any(|waker| waker.will_wake(context.waker()))
            {
                state.waiting.push(context.waker().clone());
            }
            Poll::Pending
        })
        .await;
    }

    /// The frame clock's time the app wants its next frame drawn for, and whether it owes the
    /// root something now (`Wants`), as of the last frame: at once and owed while what it
    /// placed changed and the root has yet to read it (`placed`). None and nothing once it
    /// stopped.
    pub fn wants(&self) -> Wants {
        let state = self.state.borrow();
        if state.failure.is_some() {
            return Wants::default();
        }
        let wants = wants(
            state.run.session.state(),
            state.run.session.granted_surface().is_some(),
            &state.run.pacing,
            state.now,
        );
        // Children it placed, or what its docks said, that the root has yet to read want a frame
        // now, which shows them.
        match state.placing || !state.docks.is_empty() {
            true => Wants {
                next: Some(state.now),
                owed: true,
            },
            false => wants,
        }
    }

    /// The layer showing `screen` with its top left at `origin` in its window's points, once
    /// the app painted it: the surface it painted, by handle, scaled from its pixels to the
    /// screen's points. Its id is the same every frame for the same instance and screen.
    pub fn layer(&self, screen: ScreenId, origin: Pos2) -> Option<Layer> {
        let state = self.state.borrow();
        let placement = state.painted.get(&screen)?;
        let points = 1.0 / raster(placement);
        let size = (placement.width, placement.height);
        Some(Layer {
            id: screen_layer(self.instance, screen),
            surface: Surface::Screen {
                instance: self.instance,
                surface: placement.surface,
            },
            size,
            transform: Affine::from_axes(vec2(points, 0.0), vec2(0.0, points), origin),
            clip: Rect::from_min_max(pos2(0.0, 0.0), pos2(size.0 as f32, size.1 as f32)),
            corner_radius: 0.0,
            opacity: 1.0,
            shown: true,
            input: LayerInput::default(),
        })
    }

    /// The input method's area the screen showing `surface` asks for, in its own points, and
    /// the pixels per point the surface is painted at: the field its app edits there, if any.
    pub(crate) fn ime(&self, surface: u32) -> Option<(ImeArea, f32)> {
        let state = self.state.borrow();
        let placement = state
            .painted
            .values()
            .find(|placement| placement.surface == surface)?;
        let area = state.windows.get(&placement.screen)?.ime.clone()?;
        Some((area, raster(placement)))
    }

    /// What the screen showing `surface` asks of its window (its cursor, the pointer lock,
    /// every sample, its input method's area), as its last layout applied said.
    pub(crate) fn window(&self, surface: u32) -> Option<WindowRequests> {
        let state = self.state.borrow();
        let placement = state
            .painted
            .values()
            .find(|placement| placement.surface == surface)?;
        state.windows.get(&placement.screen).cloned()
    }

    /// Sends the app `input` routed to its surface `surface`, its positions in the surface's
    /// pixels, sampled at the frame clock's `time` in nanoseconds if the platform said: the
    /// screen showing it hears it in its own points, at that time, and the app is asked for a
    /// frame. Once the screen went, only what ends a press or the focus still reaches it, so
    /// the app holds nothing pressed.
    pub(crate) fn input(&self, surface: u32, input: Input, time: Option<u64>) {
        let mut state = self.state.borrow_mut();
        // An app that does not run yet hears only where the keyboard is, state it is told once it
        // runs: every key, press and move is dropped, never held for when it runs
        // (decisions/keys_are_never_held.md).
        let running = state.run.session.running();
        if state.failure.is_some() || (!running && !matches!(input, Input::Focus(_))) {
            return;
        }
        let shown = state
            .painted
            .values()
            .any(|placement| placement.surface == surface);
        if !shown && !ends(&input) {
            return;
        }
        let Some(placement) = state.surfaces.get(&surface) else {
            return;
        };
        let every_sample = state
            .windows
            .get(&placement.screen)
            .is_some_and(|window| window.every_sample);
        let mut batch = InputBatch::new(placement.screen, every_sample);
        batch.push(TimedInput {
            input: input.scaled(1.0 / raster(placement)),
            // The app's clock is the root's frame clock, in microseconds.
            time_micros: time.map(|nanos| nanos / 1000),
            over: None,
        });
        let send = state
            .run
            .send_for_frame(vec![AppCall::Input(batch).into()], Some);
        state.wire.send(send);
        state.flush();
    }
}

/// Spawns the staged app `app`'s entry point `module` in the root's sandbox.
fn instance(app: String, module: String) -> Rc<Instance> {
    Rc::new(Instance::new(
        Source::App(StagedModule {
            app: app.clone(),
            module,
        }),
        Config {
            name: app,
            memory_limit: APP_MEMORY,
        },
    ))
}

/// What an app wants of the frames its root draws (`Embedded::wants`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wants {
    /// The frame clock's time it wants its next frame drawn for: while it starts or shuts down,
    /// by its frame's timeout (its hello or its going wakes the root first); once it runs and
    /// draws, at once while a frame it needs has not been asked for or it animates, by the
    /// timeout while one is in flight, or when it said to draw again. None for an app with no
    /// surface, which is never asked for a frame, or one that closed.
    pub next: Option<Duration>,
    /// Whether it owes the root something now: it is starting or shutting down, or draws and has
    /// a frame due or in flight. A frame it wants only later, or for an animation, is not owed,
    /// so a host waiting for its root to settle does not wait for it.
    pub owed: bool,
}

/// What an app in `session` wants at the frame clock's `now`, `drawing` when it was granted a
/// surface, as its `pacing` says.
pub(crate) fn wants(
    session: &SessionState,
    drawing: bool,
    pacing: &Pacing,
    now: Duration,
) -> Wants {
    match session {
        SessionState::Idle | SessionState::Starting | SessionState::ShuttingDown => Wants {
            next: Some(now.saturating_add(FRAME_TIMEOUT)),
            owed: true,
        },
        SessionState::Closed | SessionState::Failed(_) => Wants::default(),
        // Only an app with a surface is ever asked for a frame (`State::ask`).
        SessionState::Running if !drawing => Wants::default(),
        SessionState::Running => {
            let due = pacing.drawing() && !pacing.in_flight();
            let next = match due || pacing.animating() {
                true => Some(now),
                false => pacing
                    .wake_after(now, drawing)
                    .map(|after| now.saturating_add(after)),
            };
            Wants {
                next,
                owed: pacing.drawing(),
            }
        }
    }
}

impl State {
    /// Takes in the children each view placed, in place of what it placed before.
    fn place(&mut self, children: Vec<ChildPlacements>) {
        for placements in children {
            let view = (placements.instance, placements.view);
            let before = self
                .placed
                .iter()
                .position(|placed| (placed.instance, placed.view) == view);
            let changed = match before {
                Some(at) => self.placed[at] != placements,
                None => !placements.children.is_empty(),
            };
            if !changed {
                continue;
            }
            self.placing = true;
            if let Some(at) = before {
                self.placed.remove(at);
            }
            if !placements.children.is_empty() {
                self.placed.push(placements);
            }
        }
    }

    /// Takes in what its docks said, after what it placed with them.
    fn hear_docks(&mut self, docks: Vec<DockMessage>) {
        self.docks.extend(docks);
    }

    /// Asks the app for a frame if its pacing says one is due.
    fn ask(&mut self) -> Ask {
        let ask = self.run.pacing.ask(Asking {
            drawing: self.run.session.granted_surface().is_some(),
            seen: true,
            now: self.now,
            pass: self.pass,
            held: false,
            holds_input: self.wire.holds_input(),
        });
        if ask == Ask::Draw {
            // The root has no wall clock of its own yet: the frame clock stands in.
            let micros = u64::try_from(self.now.as_micros()).unwrap_or(u64::MAX);
            self.wire.app_call(AppCall::Draw {
                now_micros: micros,
                wall_micros: micros,
                seen: true,
            });
            self.flush();
        }
        ask
    }

    /// Takes the frames the app sent, answers what it asked, and sends what that readied.
    fn receive(&mut self, frames: Vec<Vec<u8>>) {
        let mut heard = Vec::new();
        for frame in frames {
            if let Err(failure) = self.wire.receive(&frame, &mut heard) {
                self.run
                    .session
                    .fail(SessionFailure::Protocol(failure.clone()));
                self.fail(failure);
                return;
            }
        }
        self.forward();
        for heard in heard {
            self.heard(heard);
        }
        self.failed_session();
        self.flush();
        self.wake();
    }

    fn heard(&mut self, heard: Heard) {
        // What the root serves no app yet is refused at once, so nothing the app asks waits on
        // it, but the shell's `tell`, which it takes and does nothing with. What an editor its
        // host opened asks never reaches here: the wire leaves it to the host (`forward`).
        let heard = match heard {
            Heard::Shell(ShellCall::Tell { reply, .. }) => {
                let (op, outcome) = reply.answer(&());
                self.wire.answer(op, outcome);
                return;
            }
            heard => match self.wire.refuse_unserved(heard) {
                Some(heard) => heard,
                None => return,
            },
        };
        match heard {
            Heard::Hello(hello) => {
                self.started = true;
                let accepted = self.run.session.connect(&hello, self.run.preferences());
                self.wire.accept(&accepted);
                let opening = std::mem::take(&mut self.opening);
                if let Err(failure) = self.wire.relay_forwarded(opening) {
                    self.fail(failure);
                    return;
                }
                let queued = self.run.deliverable();
                self.wire.send(queued);
                self.tell();
            }
            Heard::Layout(layout) => {
                let layout = self.run.place(layout, |_| false);
                let laid_out = |screen: &ScreenId| {
                    layout
                        .screens
                        .iter()
                        .any(|placement| placement.screen == *screen)
                };
                self.painted.retain(|screen, _| laid_out(screen));
                self.windows.retain(|screen, _| laid_out(screen));
                self.contents.retain(|screen, _| laid_out(screen));
                self.run.lay_out(layout, |_| false);
            }
            Heard::LaidOut(mut commit) => {
                // Children that only move are placed at once; any that come or go wait for the
                // present that follows, which draws the holes they fill.
                let placed = &self.placed;
                self.run
                    .holds
                    .heard_layout(&mut commit, |placements| only_moves(placed, placements));
                let children = std::mem::take(&mut commit.children);
                self.place(children);
                // Held with its children when they wait for the present, else heard now.
                self.hear_docks(std::mem::take(&mut commit.docks));
                for report in &commit.reports {
                    self.contents.insert(report.screen, report.content);
                }
                self.run.pacing.laid_out();
            }
            Heard::Presented(commit) => {
                self.run.holds.heard_presents(1);
                for held in self.run.holds.release_presented() {
                    if let Held::Laid(laid) = held {
                        for window in laid.windows {
                            self.windows.insert(window.screen, window.window);
                        }
                        self.place(laid.children);
                        self.hear_docks(laid.docks);
                    }
                }
                if let Some(presented) = commit.presented {
                    self.run.holds.heard_painted(presented.sequence);
                    for placement in presented.painted {
                        self.surfaces.insert(placement.surface, placement.clone());
                        self.painted.insert(placement.screen, placement);
                    }
                }
                self.run.pacing.presented(
                    self.now,
                    commit.animating,
                    commit.repaint_after_micros.map(Duration::from_micros),
                    commit.input_waiting,
                );
            }
            Heard::FrameNeeded => self.run.pacing.need(),
            Heard::Paintings(painted) => {
                self.painted_answer = Some(painted);
                self.wake();
            }
            Heard::ShutDown => self.run.session.shut_down(),
            Heard::Unanswered(failure) => {
                self.run
                    .session
                    .fail(SessionFailure::Protocol(failure.clone()));
                self.fail(failure);
            }
            // The root bundles no fonts and offers no published values yet; a watch it never
            // sets waits until the app drops it.
            Heard::MissingCharacters(_)
            | Heard::Watch(..)
            | Heard::Unwatch(..)
            | Heard::Ended(_)
            | Heard::Withdrawn(_)
            | Heard::Answer(..)
            | Heard::Shell(_)
            | Heard::Blocks(..)
            | Heard::Service(_)
            | Heard::Network(..)
            | Heard::Debug(..)
            | Heard::Published(..) => {}
        }
    }

    /// Tells the app what it has yet to hear of what every app is told once it runs: the fonts
    /// its root's host bundles. The root asks its apps to describe nothing, keeps the
    /// preferences it said hello with, and has no UTC offset from its host yet, so it tells
    /// UTC.
    fn tell(&mut self) {
        let fonts = Rc::clone(&self.fonts);
        let telling = Telling {
            fonts: || (*fonts).clone(),
            describing: false,
            preferences: self.run.preferences(),
            utc_offset: 0,
        };
        let told = self.run.tell(telling);
        let send = self
            .run
            .send_for_frame(told.into_iter().map(Into::into).collect(), Some);
        self.wire.send(send);
    }

    /// Hands the root's host what the app sent it, under the editor the host opened
    /// (`host.forward`). The call's `Start` goes out as it is made, before the task awaiting
    /// it is spawned (be-wasm-ops' `guest::call`), so the host hears each app's frames in the
    /// order it sent them.
    fn forward(&mut self) {
        let frames = self.wire.take_forwarded();
        if frames.is_empty() {
            return;
        }
        let forwarded = host::forward(self.instance.op(), frames);
        be_wasm_ops::guest::spawn(async move {
            // A host that stopped answering stops the root too.
            let _ = forwarded.await;
        });
    }

    /// Relays the app what its root's host sent it (`host.forwarded`).
    fn relay(&mut self, frames: Vec<Vec<u8>>) {
        if self.failure.is_some() {
            return;
        }
        if let Err(failure) = self.wire.relay_forwarded(frames) {
            self.fail(failure);
            return;
        }
        self.flush();
        self.wake();
    }

    /// Notes a session that failed, as one that timed out starting does.
    fn failed_session(&mut self) {
        if self.failure.is_none()
            && let SessionState::Failed(failure) = self.run.session.state()
        {
            let failure = format!("the app's session failed: {failure:?}");
            self.fail(failure);
        }
    }

    fn fail(&mut self, failure: String) {
        self.failure.get_or_insert(failure);
        self.wake();
    }

    /// Sends the app what the wire has for it. Nothing waits for the send's answer: the instance
    /// takes frames in the order they are sent.
    fn flush(&mut self) {
        let frames = self.wire.take_outbound();
        if !frames.is_empty() {
            self.instance.send(frames).detach();
        }
    }

    fn wake(&mut self) {
        for waker in self.waiting.drain(..) {
            waker.wake();
        }
    }
}

thread_local! {
    /// The apps whose editors the root's host opened, by their instances' handles, which hear
    /// what the host sends them (`host.forwarded`).
    static RELAYS: RefCell<HashMap<u64, Weak<RefCell<State>>>> = RefCell::default();
    /// Whether a task hears what the root's host sends its apps.
    static HEARING_HOST: Cell<bool> = const { Cell::new(false) };
}

/// Relays the app `state` runs as `handle` what the root's host sends it, hearing the host
/// from now on if nothing did yet.
fn relay_to(handle: u64, state: &Rc<RefCell<State>>) {
    RELAYS.with_borrow_mut(|relays| {
        relays.retain(|_, state| state.strong_count() > 0);
        relays.insert(handle, Rc::downgrade(state));
    });
    if !HEARING_HOST.replace(true) {
        be_wasm_ops::guest::spawn(hear_host());
    }
}

/// Hears what the root's host sends the apps it spawned (`host.forwarded`) and relays each its
/// own, until the host stops answering.
async fn hear_host() {
    while let Ok(relayed) = host::forwarded().await {
        for relayed in relayed {
            let state =
                RELAYS.with_borrow(|relays| relays.get(&relayed.instance).and_then(Weak::upgrade));
            // An app the root let go of is stopped: what the host still sent it goes nowhere.
            if let Some(state) = state {
                state.borrow_mut().relay(relayed.frames);
            }
        }
    }
    HEARING_HOST.set(false);
}

/// Hears the app until it stops or the last `Embedded` of it goes: then it lets go of the
/// instance, which stops it.
async fn hear(state: Weak<RefCell<State>>, instance: Rc<Instance>, stop: Rc<Stop>) {
    loop {
        let next = {
            let mut next = pin!(instance.next());
            std::future::poll_fn(|context| {
                if stop.stopped.get() {
                    return Poll::Ready(None);
                }
                *stop.waker.borrow_mut() = Some(context.waker().clone());
                next.as_mut().poll(context).map(Some)
            })
            .await
        };
        let (Some(next), Some(state)) = (next, state.upgrade()) else {
            return;
        };
        let mut state = state.borrow_mut();
        match next {
            Ok(Ok(frames)) => state.receive(frames),
            Ok(Err(failure)) => {
                state.fail(format!("the app stopped: {failure}"));
                return;
            }
            Err(error) => {
                state.fail(format!("the host stopped answering for the app: {error:?}"));
                return;
            }
        }
    }
}

/// Whether `placements` only moves the children its view placed before, as be-host-runtime's
/// `Instances::only_moves` says: the same children, in the same order, modes and layers.
fn only_moves(placed: &[ChildPlacements], placements: &ChildPlacements) -> bool {
    let Some(current) = placed
        .iter()
        .find(|placed| (placed.instance, placed.view) == (placements.instance, placements.view))
    else {
        return placements.children.is_empty();
    };
    let kept = |child: &ChildPlacement| {
        (
            child.child,
            child.mode,
            child.own_frame,
            child.top_bar,
            child.layer,
            child.intrinsic,
        )
    };
    current.children.len() == placements.children.len()
        && current
            .children
            .iter()
            .zip(&placements.children)
            .all(|(current, next)| kept(current) == kept(next))
}

/// Whether `input` ends a press or the focus: what a screen that went still hears.
fn ends(input: &Input) -> bool {
    matches!(
        input,
        Input::PointerButton { pressed: false, .. }
            | Input::Touch {
                phase: TouchPhase::End | TouchPhase::Cancel,
                ..
            }
            | Input::Pen {
                phase: PenPhase::Up(_) | PenPhase::Gone,
                ..
            }
            | Input::PointersCancelled
            | Input::PointerLeft
            | Input::Focus(false)
    )
}

/// The pixels per point a screen is painted at.
fn raster(placement: &ScreenPlacement) -> f32 {
    match placement.raster_millis {
        0 => 1.0,
        millis => millis as f32 / 1000.0,
    }
}

/// The layer showing an instance's screen: hashed from both under a tag of its own, as
/// `LayerId::root` is, so it is no other layer's id.
fn screen_layer(instance: u64, screen: ScreenId) -> LayerId {
    LayerId(std::hash::BuildHasher::hash_one(
        &be_lib_stable_hash::State::default(),
        ("screen", instance, screen.0),
    ))
}

fn millis(now: Duration) -> u64 {
    u64::try_from(now.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
