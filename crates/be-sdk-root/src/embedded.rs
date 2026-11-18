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
    Fonts, ImeArea, InputBatch, Opening, Preferences, ScreenId, ScreenPlacement, ScreenRequest,
    SurfaceSpec, TimedInput, WindowRequests, embed::shell::Call as ShellCall,
};
use be_sdk_app_host::{
    calls::{AppCall, Heard, Outbound},
    holds::Held,
    pacing::{Ask, Asking, FRAME_TIMEOUT, Pacing},
    run::{Run, Telling},
    session::{Session, SessionFailure, SessionState},
    wire::Wire,
};
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
        let instance = Rc::new(Instance::new(
            Source::App(StagedModule {
                app: spawn.app.clone(),
                module: spawn.module,
            }),
            Config {
                name: spawn.app,
                memory_limit: APP_MEMORY,
            },
        ));
        let mut run = Run::new(
            Session::new(HOST_NAME, Some(spawn.surface)),
            false,
            Preferences::default(),
        );
        run.session.start(millis(now));
        run.screens.clone_from(&spawn.screens);
        // Held until the hello: the session does not run yet.
        let _ = run.queue([
            Outbound::Open(Box::new(spawn.opening)),
            AppCall::Screens(spawn.screens).into(),
        ]);
        run.pacing.need();
        let handle = instance.op();
        let stop = Rc::new(Stop::default());
        let state = Rc::new(RefCell::new(State {
            instance: Rc::clone(&instance),
            wire: Wire::default(),
            run,
            fonts: spawn.fonts,
            now,
            pass: 0,
            painted: HashMap::default(),
            surfaces: HashMap::default(),
            windows: HashMap::default(),
            failure: None,
            waiting: Vec::new(),
            stop: Rc::clone(&stop),
        }));
        be_wasm_ops::guest::spawn(hear(Rc::downgrade(&state), instance, stop));
        Self {
            state,
            instance: handle,
        }
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
    /// root something now (`Wants`), as of the last frame. None and nothing once it stopped.
    pub fn wants(&self) -> Wants {
        let state = self.state.borrow();
        if state.failure.is_some() {
            return Wants::default();
        }
        wants(
            state.run.session.state(),
            state.run.session.granted_surface().is_some(),
            &state.run.pacing,
            state.now,
        )
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
        if state.failure.is_some() {
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
        for heard in heard {
            self.heard(heard);
        }
        self.failed_session();
        self.flush();
        self.wake();
    }

    fn heard(&mut self, heard: Heard) {
        // What the root serves no app yet is refused at once, so nothing the app asks waits on
        // it, but the shell's `tell`, which it takes and does nothing with. Blocks wait for a
        // store in the root (step 5).
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
                let accepted = self.run.session.connect(&hello, self.run.preferences());
                self.wire.accept(&accepted);
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
                self.run.lay_out(layout, |_| false);
            }
            Heard::LaidOut(mut commit) => {
                // The root places no children of its apps yet, so what the holds keep for the
                // present that follows is let go once that present comes.
                self.run.holds.heard_layout(&mut commit, |_| false);
                self.run.pacing.laid_out();
            }
            Heard::Presented(commit) => {
                self.run.holds.heard_presents(1);
                for held in self.run.holds.release_presented() {
                    if let Held::Laid(laid) = held {
                        for window in laid.windows {
                            self.windows.insert(window.screen, window.window);
                        }
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
            Heard::ShutDown => self.run.session.shut_down(),
            Heard::Unanswered(failure) => {
                self.run
                    .session
                    .fail(SessionFailure::Protocol(failure.clone()));
                self.fail(failure);
            }
            // The root bundles no fonts, offers no published values and asks for no paintings
            // yet; a watch it never sets waits until the app drops it.
            Heard::MissingCharacters(_)
            | Heard::Watch(..)
            | Heard::Unwatch(..)
            | Heard::Ended(_)
            | Heard::Withdrawn(_)
            | Heard::Answer(..)
            | Heard::Paintings(_)
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
