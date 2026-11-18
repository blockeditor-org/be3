//! One app a root guest spawned, run as its host: the app protocol's host half over the
//! instance's channel.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    task::{Poll, Waker},
    time::Duration,
};

use be_lib_geometry::{Pos2, Rect, pos2, vec2};
use be_lib_layers::{Affine, Layer, LayerId, LayerInput, Surface};
use be_lib_stable_hash::HashMap;
use be_lib_ui_types::input::Input;
use be_sdk_api::{
    InputBatch, Opening, Preferences, ScreenId, ScreenPlacement, ScreenRequest, SurfaceSpec,
    embed::shell::Call as ShellCall,
    ops::{OpError, Outcome},
};
use be_sdk_app_host::{
    calls::{AppCall, Heard, Outbound},
    pacing::{Ask, Asking},
    run::Run,
    session::{Session, SessionFailure, SessionState},
    wire::{Service, Wire},
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
}

/// An app the root spawned and runs as its host. Clones are the same app. It stops when the
/// last clone goes and the app next sends anything, or with the root.
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
    /// The frame clock's time, as the last frame was asked for.
    now: Duration,
    /// The root's frames so far, the pass the app's pacing counts.
    pass: u64,
    /// A `Draw` was sent and the app has yet to present it.
    drawing: bool,
    /// What each screen showed when the app last painted it.
    painted: HashMap<ScreenId, ScreenPlacement>,
    /// Why the app stopped, once it did: it is asked for nothing again.
    failure: Option<String>,
    /// The frames waiting for the app to say hello or present.
    waiting: Vec<Waker>,
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
        let state = Rc::new(RefCell::new(State {
            instance: Rc::clone(&instance),
            wire: Wire::default(),
            run,
            now,
            pass: 0,
            drawing: false,
            painted: HashMap::default(),
            failure: None,
            waiting: Vec::new(),
        }));
        be_wasm_ops::guest::spawn(hear(Rc::downgrade(&state), instance));
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
        let mut asked = false;
        std::future::poll_fn(|context| {
            let mut state = self.state.borrow_mut();
            if state.failure.is_some() {
                return Poll::Ready(());
            }
            if state.run.session.running() {
                if state.drawing {
                    asked = true;
                } else if asked {
                    return Poll::Ready(());
                } else {
                    match state.ask() {
                        Ask::Draw => asked = true,
                        Ask::AfterInput => {}
                        Ask::Nothing | Ask::Later => return Poll::Ready(()),
                    }
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

    /// Sends the app `input` routed to its surface `surface`, its positions in the surface's
    /// pixels: the screen showing it hears it in its own points, and the app is asked for a
    /// frame.
    pub(crate) fn input(&self, surface: u32, input: Input) {
        let mut state = self.state.borrow_mut();
        if state.failure.is_some() {
            return;
        }
        let Some(placement) = state
            .painted
            .values()
            .find(|placement| placement.surface == surface)
        else {
            return;
        };
        let mut batch = InputBatch::new(placement.screen, false);
        batch.push(input.scaled(1.0 / raster(placement)));
        let send = state
            .run
            .send_for_frame(vec![AppCall::Input(batch).into()], Some);
        state.wire.send(send);
        state.flush();
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
            self.drawing = true;
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
        match heard {
            Heard::Hello(hello) => {
                let accepted = self.run.session.connect(&hello, self.run.preferences());
                self.wire.accept(&accepted);
                let queued = self.run.deliverable();
                self.wire.send(queued);
            }
            Heard::Layout(layout) => {
                let layout = self.run.place(layout, |_| false);
                self.painted.retain(|screen, _| {
                    layout
                        .screens
                        .iter()
                        .any(|placement| placement.screen == *screen)
                });
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
                drop(self.run.holds.release_presented());
                if let Some(presented) = commit.presented {
                    self.run.holds.heard_painted(presented.sequence);
                    for placement in presented.painted {
                        self.painted.insert(placement.screen, placement);
                    }
                }
                self.run.pacing.presented(
                    self.now,
                    commit.animating,
                    commit.repaint_after_micros.map(Duration::from_micros),
                    commit.input_waiting,
                );
                self.drawing = false;
            }
            Heard::FrameNeeded => self.run.pacing.need(),
            Heard::ShutDown => self.run.session.shut_down(),
            Heard::Unanswered(failure) => {
                self.run
                    .session
                    .fail(SessionFailure::Protocol(failure.clone()));
                self.fail(failure);
            }
            // What the root serves no app yet: each is refused at once, so nothing the app asks
            // waits on it. Blocks wait for a store in the root (step 5).
            Heard::Shell(ShellCall::Tell { reply, .. }) => {
                let (op, outcome) = reply.answer(&());
                self.wire.answer(op, outcome);
            }
            Heard::Shell(call) => {
                let (op, outcome) = call.refuse(OpError::Unsupported);
                self.wire.answer(op, outcome);
            }
            Heard::Blocks(_, call) => {
                let (op, outcome) = call.refuse(OpError::Unsupported);
                self.wire.answer(op, outcome);
            }
            Heard::Service(Service { call, .. }) => {
                let (op, outcome) = call.refuse(OpError::Unsupported);
                self.wire.answer(op, outcome);
            }
            Heard::Network(_, call) => {
                let (op, outcome) = call.refuse(OpError::Unsupported);
                self.wire.answer(op, outcome);
            }
            Heard::Debug(_, call) => {
                let (op, outcome) = call.refuse(OpError::Unsupported);
                self.wire.answer(op, outcome);
            }
            Heard::Published(_, started) => {
                self.wire
                    .answer(started.op, Outcome::Err(OpError::Unsupported));
            }
            // The root bundles no fonts, offers no published values and asks for no paintings
            // yet; a watch it never sets waits until the app drops it.
            Heard::MissingCharacters(_)
            | Heard::Watch(..)
            | Heard::Unwatch(..)
            | Heard::Ended(_)
            | Heard::Withdrawn(_)
            | Heard::Answer(..)
            | Heard::Paintings(_) => {}
        }
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
        self.drawing = false;
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

/// Hears the app until it stops or the last `Embedded` of it goes.
async fn hear(state: Weak<RefCell<State>>, instance: Rc<Instance>) {
    loop {
        let next = instance.next().await;
        let Some(state) = state.upgrade() else {
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
