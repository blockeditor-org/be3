use be_lib_stable_hash::HashMap;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use be_sdk_api::{
    AnchorSpace, ArtifactDescription, BlockPick, ChildPreview, EditorInstanceId, FrameSpec,
    HostPanel, HostTransform, LayoutCommit, OpeningKind, PresentCommit, PreviewImage, ScreenDamage,
    ScreenId, ScreenLayout, ScreenPlacement, SurfaceFormat, SurfaceSpec, View, ViewId,
    WindowRequest,
};
use be_sdk_app_host::{
    holds::{Held, Laid},
    pacing::{self, FrameClock},
    run::{Run, Telling},
};
use be_sdk_manifest::AppManifest;
use beui::native::EmbedPlacement;
use beui::{Pos2, Rect, Vec2, pos2, vec2};

use uuid::Uuid;

use be_host_base::host;
use be_sdk_api::app::{EditorEvent, MenuChoice};

use super::{
    ArtifactSlot, ArtifactState, CreationSlot, CreationState, EditorBlock, HostAnchored, HostChild,
    HostChildStatus, InstanceRole, RuntimeStatus, SurfaceStatus,
    backend::{Availability, Backend, Deadline, Frame, ShownFrame},
    calls::{AppCall, EditorAnswer, EditorCall, Heard, Outbound},
    input::{BlockDrag, DragEvent},
    instances::{DragKind, EditorView, Focus, Instances, Placement},
    presenter::{
        self, AnchoredBlits, AnchoredDrawing, AnchoredSource, Blit, Piece, PresenterState,
        PresenterStatus, Quad, Shared, ViewDrawing,
    },
    preview_scale, previews,
    session::{Session, SessionFailure, SessionState},
};

const HOST_NAME: &str = "BE3";
const UNIT: Rect = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
use be_host_base::performance::PACING;
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::new());
    static STEPS: RefCell<BTreeMap<String, beui::native::FrameTimes>> = const { RefCell::new(BTreeMap::new()) };
}

pub fn take_steps() -> Vec<(String, beui::native::FrameTimes)> {
    STEPS.with(|steps| {
        std::mem::take(&mut *steps.borrow_mut())
            .into_iter()
            .collect()
    })
}

struct Host {
    availability: Availability,
    clock: FrameClock,
    /// What the frame clock's real times count from.
    origin: Instant,
    runtimes: HashMap<String, Runtime>,
    warming: Option<Vec<AppManifest>>,
    focus: Focus,
    shell: Option<(String, EditorInstanceId)>,
    grabbed: bool,
}

impl Host {
    fn new() -> Self {
        be_host_base::watches::serve(&Watches);
        Self {
            availability: Availability::missing(),
            clock: FrameClock::default(),
            origin: Instant::now(),
            runtimes: HashMap::default(),
            warming: Some(Vec::new()),
            focus: Focus::default(),
            shell: None,
            grabbed: false,
        }
    }

    fn runtime(&mut self, app: &AppManifest) -> Result<&mut Runtime, String> {
        if let Err(error) = &self.availability.0 {
            return Err(error.clone());
        }
        let surface = self.surface_for(&app.identity.id);
        let focus = self.focus.clone();
        let shell = self
            .shell
            .as_ref()
            .filter(|(shell, _)| *shell == app.identity.id)
            .map(|(_, instance)| *instance);
        let runtime = self
            .runtimes
            .entry(app.identity.id.clone())
            .or_insert_with(|| {
                let mut runtime = Runtime::new(app, surface);
                for (open, value) in be_host_base::watches::published() {
                    runtime.instances.publish(open, value);
                }
                runtime.instances.set_shell(shell);
                runtime
            });
        runtime.instances.set_focus(focus);
        let pass = host::pass();
        runtime.contain(|runtime| runtime.begin_pass(pass));
        Ok(runtime)
    }

    fn surface_for(&self, app_id: &str) -> u32 {
        if let Some(runtime) = self.runtimes.get(app_id) {
            return runtime.surface;
        }
        (0..)
            .find(|surface| {
                !self
                    .runtimes
                    .values()
                    .any(|runtime| runtime.surface == *surface)
            })
            .expect("fewer runtimes than surface ids")
    }

    fn stop_idle(&mut self, now: Duration) -> Option<Duration> {
        let mut wake: Option<Duration> = None;
        let mut idle = Vec::new();
        for (app_id, runtime) in &mut self.runtimes {
            if !runtime.instances.is_empty() {
                runtime.idle_since = None;
                continue;
            }
            let since = *runtime.idle_since.get_or_insert(now);
            match (since + IDLE_TIMEOUT).checked_sub(now) {
                Some(left) if !left.is_zero() => {
                    wake = Some(wake.map_or(left, |wake| wake.min(left)));
                }
                _ => idle.push(app_id.clone()),
            }
        }
        for app_id in idle {
            self.shutdown(&app_id);
            mark(&app_id);
        }
        wake
    }

    fn shutdown(&mut self, app_id: &str) {
        let Some(mut runtime) = self.runtimes.remove(app_id) else {
            return;
        };
        runtime.stop();
        if runtime.error.is_none() {
            heard_stop(app_id);
        }
        presenter::release(runtime.surface, &runtime.status);
        host::request_repaint();
    }
}

/// Asks the `asked` runtimes for the frames now due, holding a child whose placer draws in this
/// frame until the placer's layout commit lands, so it is asked once, at its new place.
fn request_frames(runtimes: &mut HashMap<String, Runtime>, asked: &[String]) {
    let active: be_lib_stable_hash::HashSet<String> = runtimes
        .iter()
        .filter(|(id, runtime)| {
            runtime.run.pacing.laying_out(host::pass())
                || (asked.contains(*id) && runtime.will_draw())
        })
        .map(|(id, _)| id.clone())
        .collect();
    let held = held_behind(runtimes, &active);
    for id in asked {
        let held = held.contains(id.as_str());
        if let Some(runtime) = runtimes.get_mut(id) {
            runtime.contain(|runtime| runtime.request_frame(held));
        }
    }
}

fn held_behind(
    runtimes: &HashMap<String, Runtime>,
    active: &be_lib_stable_hash::HashSet<String>,
) -> be_lib_stable_hash::HashSet<String> {
    let held = pacing::held_behind(
        runtimes
            .iter()
            .filter(|(id, _)| active.contains(*id))
            .flat_map(|(id, runtime)| {
                runtime
                    .placers
                    .iter()
                    .filter(|(_, placer)| {
                        placer.app != *id
                            && active.contains(&placer.app)
                            && runtimes.get(&placer.app).is_some_and(|runtime| {
                                runtime.instances.mounted(placer.instance, placer.view)
                            })
                    })
                    .map(move |((instance, view), placer)| {
                        match runtime.instances.fits(*instance, *view) {
                            true => (*instance, id.as_str(), placer.app.as_str()),
                            false => (*instance, placer.app.as_str(), id.as_str()),
                        }
                    })
            })
            .collect(),
    );
    held.into_iter().map(str::to_owned).collect()
}

/// The runtimes a settle asked, as the library's settling steps and waits on them.
struct Asked<'a> {
    runtimes: &'a mut HashMap<String, Runtime>,
    asked: &'a [String],
    origin: Instant,
}

impl pacing::Settling for Asked<'_> {
    fn pump(&mut self) -> bool {
        let mut relaid = false;
        for id in self.asked {
            if let Some(runtime) = self.runtimes.get_mut(id) {
                runtime.contain(Runtime::pump);
                relaid |= runtime.take_relaid();
            }
        }
        relaid
    }

    fn request_frames(&mut self) {
        request_frames(self.runtimes, self.asked);
    }

    fn outstanding(&self) -> bool {
        self.asked.iter().any(|id| {
            self.runtimes
                .get(id)
                .is_some_and(|runtime| runtime.outstanding())
        })
    }

    fn wait(&mut self, rung: u64, deadline: Duration) -> bool {
        be_wasm_guest_host::bell::wait(rung, Deadline::at(self.origin + deadline))
    }

    fn rung(&self) -> u64 {
        be_wasm_guest_host::bell::rung()
    }

    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
}

pub(super) struct Runtime {
    app: AppManifest,
    pub(super) backend: Backend,
    /// What the app's module said and was told since it last started.
    run: Run<Answer>,
    /// The person asked to restart the app; the next `poll` does.
    restart_asked: bool,
    taking_in: bool,
    /// A frame held until the poll applies the children and docks its layout brought
    /// (`Runtime::received`), so both show in the same host frame.
    held_frame: Option<Frame>,
    pub(super) instances: Instances,
    pub(super) pass: u64,
    since: u64,
    placed: u64,
    surface: u32,
    status: PresenterStatus,
    shared: Rc<RefCell<Shared>>,
    presented: bool,
    error: Option<String>,
    pub(super) placements: HashMap<(EditorInstanceId, ViewId), Placed>,
    offers: HashMap<(EditorInstanceId, ViewId), Vec<Offer>>,
    fallbacks: super::fonts::Fallbacks,
    frames: u64,
    steps: u64,
    relaid: bool,
    awaiting_input: bool,
    drawn: HashMap<ScreenId, (u32, u32)>,
    placers: HashMap<(EditorInstanceId, ViewId), Placer>,
    idle_since: Option<Duration>,
}

enum Answer {
    Layout(ScreenLayout),
    FrameNeeded,
    MissingCharacters(Vec<char>),
    Shell(be_sdk_api::embed::shell::Call),
    Editor(EditorInstanceId, EditorAnswer),
    Paintings(Vec<u8>),
    Blocks(EditorInstanceId, be_sdk_api::embed::blocks::Call),
    Watch(EditorInstanceId, be_sdk_api::ops::MethodId),
    Unwatch(EditorInstanceId, be_sdk_api::ops::MethodId),
    Published(EditorInstanceId, be_sdk_api::ops::Started),
    Debug(EditorInstanceId, be_sdk_api::embed::debug::debug::Call),
    Ended(be_sdk_api::ops::OpId),
    Service(super::wire::Service),
    Network(EditorInstanceId, be_sdk_api::embed::network::Call),
    Withdrawn(be_sdk_api::ops::OpId),
    LaidOut(LayoutCommit),
    Presented(PresentCommit),
    Laid(Laid),
}

impl From<Heard> for Answer {
    fn from(heard: Heard) -> Self {
        match heard {
            Heard::Layout(layout) => Self::Layout(layout),
            Heard::FrameNeeded => Self::FrameNeeded,
            Heard::MissingCharacters(missing) => Self::MissingCharacters(missing),
            Heard::LaidOut(commit) => Self::LaidOut(*commit),
            Heard::Presented(presented) => Self::Presented(presented),
            Heard::Hello(_) | Heard::ShutDown | Heard::Unanswered(_) => {
                unreachable!("the session takes what it hears of its own")
            }
            Heard::Shell(call) => Self::Shell(call),
            Heard::Answer(instance, answer) => Self::Editor(instance, answer),
            Heard::Paintings(payload) => Self::Paintings(payload),
            Heard::Blocks(instance, call) => Self::Blocks(instance, call),
            Heard::Watch(instance, open, _) => Self::Watch(instance, open),
            Heard::Unwatch(instance, open) => Self::Unwatch(instance, open),
            Heard::Published(instance, started) => Self::Published(instance, started),
            Heard::Debug(instance, call) => Self::Debug(instance, call),
            Heard::Ended(op) => Self::Ended(op),
            Heard::Service(service) => Self::Service(service),
            Heard::Network(instance, call) => Self::Network(instance, call),
            Heard::Withdrawn(op) => Self::Withdrawn(op),
        }
    }
}

/// Whether a present's pixels show only once the host draws a frame (natively, where it holds the
/// app's textures), so a kept or closing frame waits for that frame (`Shown`). On the web the app's
/// worker shows each present on its canvases as it makes it, and the page keeps a hidden view's
/// canvas (`web::canvases`).
const DRAWS_PRESENTS: bool = cfg!(not(target_arch = "wasm32"));

pub(super) struct Placed {
    pub(super) rect: Rect,
    pub(super) clip: Rect,
    pub(super) zoom: f32,
    pub(super) inert: bool,
    shown: Rect,
}

/// Readies `messages` for `app`'s wire: offers the previews they carry, holds what waits for a
/// report, and translates their ids and pastes.
fn ready(
    app: &str,
    instances: &mut Instances,
    backend: &mut Backend,
    messages: Vec<Outbound>,
) -> Vec<Outbound> {
    offer_previews(app, backend, &messages);
    let mut messages = instances.hold_unreported(messages);
    for message in &mut messages {
        instances.translate_pastes(message);
        instances.translate(message, false);
    }
    instances.gate(messages)
}

fn offer_previews(app: &str, backend: &mut Backend, messages: &[Outbound]) {
    for message in messages {
        let Outbound::Editor(_, EditorCall::ChildStatuses(statuses)) = message else {
            continue;
        };
        for status in statuses {
            let ChildPreview::Image(image) = status.preview else {
                continue;
            };
            if let Some(picture) = previews::offer(app, image.image) {
                backend.offer(image.image, &picture);
            }
        }
    }
}

fn frame_changed(damage: Option<&[ScreenDamage]>) -> bool {
    damage.is_none_or(|damage| !damage.is_empty())
}

impl Runtime {
    fn new(app: &AppManifest, surface: u32) -> Self {
        let mut backend = Backend::new(app);
        backend.start(app);
        let mut instances = Instances::default();
        instances.allow_network(app.network.clone());
        instances.allow_block_agents(app.block_agents);
        instances.allow_sharing(app.sharing);
        instances.set_app_id(app.identity.id.clone());
        Self {
            app: app.clone(),
            backend,
            run: Run::new(session(), DRAWS_PRESENTS, host::preferences()),
            taking_in: false,
            held_frame: None,
            instances,
            pass: 0,
            since: 0,
            placed: 0,
            surface,
            status: PresenterStatus::waiting(),
            shared: Rc::new(RefCell::new(Shared::default())),
            presented: false,
            error: None,
            restart_asked: false,
            placements: HashMap::default(),
            offers: HashMap::default(),
            fallbacks: super::fonts::Fallbacks::new(),
            frames: 0,
            steps: 0,
            relaid: false,
            awaiting_input: false,
            drawn: HashMap::default(),
            placers: HashMap::default(),
            idle_since: None,
        }
    }

    fn stop(&mut self) {
        if self.run.session.shutdown(self.milliseconds()) {
            self.backend.ask_to_shut_down();
        }
        self.backend.shutdown();
        self.instances.forget_fetches(None);
        previews::forget(&self.app.identity.id);
    }

    fn restart(&mut self) {
        mark(&self.app.identity.id);
        let app = self.app.clone();
        self.stop();
        presenter::release(self.surface, &self.status);
        self.shared.borrow_mut().frames.clear();
        self.held_frame = None;
        self.error = None;
        self.status = PresenterStatus::waiting();
        self.fallbacks = super::fonts::Fallbacks::new();
        self.drawn.clear();
        let (run, instances) = (&mut self.run, &mut self.instances);
        super::docks::with(|docks| {
            run.restart(
                session(),
                host::preferences(),
                &app.identity.id,
                docks,
                instances,
            );
        });
        self.instances.reopen();
        self.backend.start(&app);
    }

    fn begin_pass(&mut self, pass: u64) {
        self.detect_error();
        if self.pass == pass || self.error.is_some() {
            return;
        }
        self.since = self.pass;
        self.pass = pass;
        let messages = self.run.tell(Telling {
            fonts: super::fonts::bundled,
            describing: super::description::describing(),
            preferences: host::preferences(),
            utc_offset: be_host_base::platform::utc_offset(),
        });
        self.update(messages.into_iter().map(Outbound::App).collect());
    }

    fn refresh_anchor_space(&mut self, instance: EditorInstanceId, region: ViewId) {
        if self.instances.refresh_anchor_space(instance, region) {
            self.update(Vec::new());
        }
    }

    fn update(&mut self, mut messages: Vec<Outbound>) {
        let drawing = self.run.session.granted_surface().is_some();
        let next = self.instances.next_screens(self.since);
        messages.extend(next.opened);
        if drawing && self.run.screens != next.screens {
            self.run.screens.clone_from(&next.screens);
            messages.push(AppCall::Screens(self.instances.screen_set(next.screens)).into());
            messages.extend(self.instances.take_announced_input());
        }
        messages.extend(self.instances.pending());
        messages.extend(
            super::docks::take(&self.app.identity.id)
                .into_iter()
                .map(|(instance, update)| Outbound::Editor(instance, EditorCall::Dock(update))),
        );
        messages.extend(
            super::docks::take_picks(&self.app.identity.id)
                .into_iter()
                .map(|(owner, view, id, anchor)| {
                    self.instances.note_menu_pick(owner);
                    Outbound::Event(
                        owner,
                        EditorEvent::MenuPick(MenuChoice { view, id, anchor }),
                    )
                }),
        );
        messages.extend(
            super::docks::take_details(&self.app.identity.id)
                .into_iter()
                .map(|(placer, child)| Outbound::Event(placer, EditorEvent::ChildDetails(child))),
        );
        self.serve();
        messages.extend(self.instances.configure(&messages).map(Outbound::from));
        let awaited = messages.iter().any(|message| {
            matches!(
                message,
                Outbound::Open(opening)
                    if matches!(opening.kind, OpeningKind::Block(_) | OpeningKind::Artifact(_))
            )
        });
        self.send_for_frame(messages);
        self.answer();
        if awaited {
            host::request_repaint();
        }
        self.pump();
    }

    fn begin_frame(&mut self, pass: u64) {
        if self.error.is_some() || self.pass + 1 < pass {
            return;
        }
        let messages = self.instances.drive_web_views();
        self.send_for_frame(messages);
    }

    fn ask(&mut self, pass: u64) -> bool {
        if self.placed == pass || self.instances.has_mounted() || self.instances.has_hosts() {
            self.begin_pass(pass);
        }
        self.detect_error();
        if self.error.is_some() || self.pass + 1 < pass {
            return false;
        }
        match self.pass == pass {
            true => self.update(Vec::new()),
            false => self.pump(),
        }
        true
    }

    fn start(&mut self, pass: u64) -> bool {
        self.detect_error();
        if self.error.is_some() || !(self.instances.has_mounted() || self.pass + 1 >= pass) {
            return false;
        }
        self.pump();
        true
    }

    fn will_draw(&self) -> bool {
        let pass = host::pass();
        self.error.is_none()
            && self.run.session.granted_surface().is_some()
            && self.run.pacing.will_draw(host::seen(), self.now(), pass)
    }

    fn outstanding(&self) -> bool {
        self.error.is_none()
            && (!self.backend.settled() || self.run.pacing.painting() || self.awaiting_input)
    }

    fn take_relaid(&mut self) -> bool {
        std::mem::take(&mut self.relaid)
    }

    pub(super) fn name(&self) -> &str {
        &self.app.identity.name
    }

    pub(super) fn id(&self) -> &str {
        &self.app.identity.id
    }

    pub(super) fn node_action(
        &mut self,
        screen: ScreenId,
        request: beui::accesskit::ActionRequest,
    ) {
        self.send_for_frame(vec![Outbound::App(AppCall::NodeAction {
            screen,
            request: request.into(),
        })]);
        host::request_repaint();
    }

    pub(super) fn run_action(&mut self, screen: ScreenId, id: String) {
        self.send_for_frame(vec![Outbound::App(AppCall::RunAction { screen, id })]);
        host::request_repaint();
    }

    fn busy(&mut self) -> Vec<&'static str> {
        if self.error.is_some() {
            return Vec::new();
        }
        let starting = matches!(
            self.run.session.state(),
            SessionState::Idle | SessionState::Starting
        );
        // An app with nothing mounted is settled, unless it is starting for a view the host
        // asked of it (a new block's tab, say): that view mounts once the app has started, which
        // a stopped clock must not be taken for having settled before.
        if !self.instances.has_mounted() {
            return match starting && self.instances.has_views() {
                true => vec!["is starting for a view the host asked of it"],
                false => Vec::new(),
            };
        }
        [
            (starting, "is starting"),
            (self.run.painting, "has yet to answer a paint"),
            (self.drawing(), "is drawing a frame"),
            (
                self.shows_undrawn_size(),
                "shows a view at a size it has yet to draw",
            ),
            (!self.backend.settled(), "has input or a step in flight"),
            (
                !self.run.holds.is_empty(),
                "has answers it sent mid-frame for the next frame to take in",
            ),
            (
                self.instances.awaiting_content(),
                "has an editor waiting for a block's content",
            ),
        ]
        .into_iter()
        .filter_map(|(busy, reason)| busy.then_some(reason))
        .collect()
    }

    fn shows_undrawn_size(&self) -> bool {
        self.run
            .layout()
            .screens
            .iter()
            .filter(|placement| placement.view != ViewId::PREVIEW)
            .filter(|placement| self.instances.view_request_of(placement.screen).is_some())
            .any(|placement| {
                self.requested_size(placement.screen)
                    .is_some_and(|requested| {
                        requested.0 > 0
                            && requested.1 > 0
                            && self
                                .drawn
                                .get(&placement.screen)
                                .is_some_and(|drawn| *drawn != requested)
                    })
            })
    }

    fn requested_size(&self, screen: ScreenId) -> Option<(u32, u32)> {
        let request = self.instances.request_of(screen)?;
        if request.fit {
            return self
                .run
                .layout()
                .placement(screen)
                .map(|placement| (placement.width, placement.height));
        }
        let metrics = &request.metrics;
        Some(match self.run.layout().placement(screen) {
            Some(placement) => super::input::requested_at(metrics, super::input::finer(placement)),
            None => (metrics.pixel_width, metrics.pixel_height),
        })
    }

    fn request_painting(&mut self) {
        if self.error.is_some() || !self.instances.has_mounted() || !self.run.session.running() {
            return;
        }
        self.run.painting = true;
        self.send(vec![Outbound::App(AppCall::Paintings)]);
    }

    fn painted(&mut self, payload: &[u8]) {
        self.run.painting = false;
        let mut shared = self.shared.borrow_mut();
        // A kept frame's painting stays with it: the app no longer paints a view it left out.
        let kept = &self.run.shown;
        shared.paintings.retain(|screen, _| kept.keeps(*screen));
        shared.overflows.clear();
        let Ok(paintings) = beui::native::painting::ScreenPaintings::decode(payload) else {
            return;
        };
        for (screen, painting) in paintings.screens {
            shared.paintings.insert(ScreenId(screen), painting);
        }
        for (screen, overflows) in paintings.overflows {
            shared.overflows.insert(ScreenId(screen), overflows);
        }
    }

    fn drawing(&mut self) -> bool {
        if self.error.is_some() || !self.instances.has_mounted() {
            return false;
        }
        let drawing = self.run.session.granted_surface().is_some();
        drawing && self.run.pacing.drawing()
    }

    fn request_frame(&mut self, held: bool) {
        if self.error.is_some() {
            return;
        }
        let drawing = self.run.session.granted_surface().is_some();
        let seen = host::seen();
        let pass = host::pass();
        let ask = self.run.pacing.ask(pacing::Asking {
            drawing,
            seen,
            now: self.now(),
            pass,
            held,
            holds_input: self.backend.holds_input(),
        });
        self.awaiting_input = ask == pacing::Ask::AfterInput;
        match ask {
            pacing::Ask::Nothing | pacing::Ask::AfterInput => {}
            pacing::Ask::Later => host::request_repaint(),
            pacing::Ask::Draw => self.send(vec![AppCall::Draw {
                now_micros: self.now().as_micros() as u64,
                wall_micros: host::wall_clock().as_micros() as u64,
                seen,
            }]),
        }
        if let Some(delay) = self.run.pacing.wake_after(self.now(), drawing) {
            host::request_repaint_after(delay);
        }
        if drawing && self.run.pacing.animating() {
            host::note_animating();
        }
    }

    fn now(&self) -> Duration {
        host::now()
    }

    fn milliseconds(&self) -> u64 {
        host::milliseconds()
    }

    fn contain<R: Default>(&mut self, act: impl FnOnce(&mut Self) -> R) -> R {
        match be_host_base::panic_guard::contain(|| act(self)) {
            Ok(done) => done,
            Err(panic) => {
                self.crashed(format!("be-host failed while running it: {panic}"));
                R::default()
            }
        }
    }

    fn crashed(&mut self, error: String) {
        if self.error.is_some() {
            return;
        }
        let first = error.lines().next().unwrap_or_default();
        be_host_base::notices::report_with(
            format!("{} stopped: {first}", self.name()),
            error.clone(),
        );
        self.stopped(error);
    }

    fn stopped(&mut self, error: String) {
        if self.error.is_some() {
            return;
        }
        self.error = Some(error);
        self.held_frame = None;
        let _ = be_host_base::panic_guard::contain(|| self.backend.shutdown());
        self.instances.forget_fetches(None);
        previews::forget(&self.app.identity.id);
        heard_stop(&self.app.identity.id);
        super::forget_windows(&self.app.identity.id);
        mark(&self.app.identity.id);
        host::request_repaint();
    }

    fn detect_error(&mut self) {
        if self.error.is_some() {
            return;
        }
        if let Some(error) = self.backend.take_error() {
            self.crashed(error);
            return;
        }
        match self.status.get() {
            PresenterState::Failed(error) | PresenterState::Unsupported(error) => {
                self.stopped(error);
            }
            PresenterState::Waiting | PresenterState::Presenting | PresenterState::Released => {}
        }
    }

    fn serve(&mut self) {
        let answers = self.instances.take_answers();
        if !answers.is_empty() {
            self.backend.answer(answers);
        }
    }

    fn pump(&mut self) {
        if self.error.is_some() {
            return;
        }
        let now = self.milliseconds();
        let before = self.run.session.state().clone();
        let received = self.backend.receive();
        if *self.run.session.state() == SessionState::Idle && self.backend.ready() {
            self.run.session.start(now);
        }
        let mut forwarded = Vec::with_capacity(received.len());
        for heard in received {
            match heard {
                Heard::Hello(hello) => {
                    let accepted = self.run.session.connect(&hello, self.run.preferences());
                    self.backend.accept(&accepted);
                }
                Heard::ShutDown => self.run.session.shut_down(),
                Heard::Unanswered(error) => self.run.session.fail(SessionFailure::Protocol(error)),
                heard => forwarded.push(heard),
            }
        }
        self.deliver();
        self.run.session.tick(now);
        if *self.run.session.state() != before {
            mark(&self.app.identity.id);
            host::request_repaint();
        }
        if let Some(deadline) = self.run.session.next_deadline() {
            host::request_repaint_after(Duration::from_millis(deadline.saturating_sub(now)));
        }
        match self.run.session.state() {
            SessionState::Idle | SessionState::Starting | SessionState::Running => {}
            SessionState::Failed(failure) => {
                let error = format!("The app session stopped: {failure:?}");
                self.crashed(error);
                return;
            }
            state => {
                let error = format!("The app session stopped: {state:?}");
                self.stopped(error);
                return;
            }
        }
        self.hear(forwarded);
        let (count, longest) = self.backend.stepped();
        self.steps += count;
        if count > 0 {
            STEPS.with(|steps| {
                steps
                    .borrow_mut()
                    .entry(self.app.identity.id.clone())
                    .or_default()
                    .merge(beui::native::FrameTimes { count, longest });
            });
        }
        if let Some(took) = self.backend.took() {
            let id = &self.app.identity.id;
            for (name, duration) in [
                ("step", took.step),
                ("GPU calls", took.gpu),
                ("submit", took.submit),
            ] {
                be_host_base::performance::record_group_duration(
                    PACING,
                    &format!("{id} {name}"),
                    duration,
                );
            }
        }
        for image in previews::take_withdrawn(&self.app.identity.id) {
            self.backend.withdraw(image);
        }
        if let Some(frame) = self.backend.received_frame() {
            self.received(frame);
        }
    }

    /// Shows a frame the app presented, unless it may paint what held children or docks go
    /// with: shown before the poll applies them, its painting (a dock's bar showing the tab it
    /// switched to) would sit a frame over the children it replaces. It is shown once they are
    /// applied, in `hear`.
    fn received(&mut self, frame: Frame) {
        if self.waits_for_children(frame.presents()) {
            self.held_frame = Some(frame);
            host::request_repaint();
            return;
        }
        self.held_frame = None;
        self.show_frame(frame);
    }

    /// Whether the frame of present `sequence` comes after the layout of children still held.
    fn waits_for_children(&self, sequence: u64) -> bool {
        self.run.holds.waits_for_children(sequence)
    }

    fn show_frame(&mut self, mut frame: Frame) {
        let previewed = self.export_previews(&frame);
        let mut changed = true;
        if frame.composited() {
            let (damage, painted) = self.run.presents.damage_through(frame.presents());
            frame.set_damage(damage);
            self.shared.borrow_mut().paint(&painted);
            changed = frame_changed(frame.damage());
            let release = self.run.shown.frame_shown(self.run.presents.shown());
            if release.kept {
                self.release_kept();
            }
            self.shared.borrow_mut().publish(
                self.run.layout(),
                self.run.shown.kept_placements(),
                Some(frame),
            );
            self.close_anchored(release.closed);
            self.frames += 1;
            be_host_base::performance::record_group_count(
                PACING,
                &format!("{} frames", self.app.identity.id),
                self.frames,
            );
        }
        if previewed || changed {
            mark(&self.app.identity.id);
        }
    }

    fn hear(&mut self, forwarded: Vec<Heard>) {
        let presents = forwarded
            .iter()
            .filter(|heard| matches!(heard, Heard::Presented(_)))
            .count() as u64;
        self.run.holds.heard_presents(presents);
        let mut answers = Vec::with_capacity(forwarded.len());
        if self.taking_in {
            self.release_presented(&mut answers);
        }
        let mut held = false;
        for heard in forwarded {
            match heard {
                Heard::LaidOut(commit) => {
                    let mut commit = *commit;
                    self.instances.translate(&mut commit, true);
                    let instances = &self.instances;
                    held |= self
                        .run
                        .holds
                        .heard_layout(&mut commit, |placements| instances.only_moves(placements));
                    answers.push(Answer::LaidOut(commit));
                }
                Heard::Presented(commit) => {
                    if let Some(presented) = &commit.presented {
                        self.run.holds.heard_painted(presented.sequence);
                    }
                    answers.push(Answer::Presented(commit));
                }
                heard @ (Heard::Paintings(_) | Heard::Layout(_) | Heard::FrameNeeded) => {
                    answers.push(Answer::from(heard));
                }
                heard if self.taking_in && self.run.holds.is_empty() => {
                    answers.push(Answer::from(heard));
                }
                heard => {
                    held = true;
                    self.run.holds.hold(Answer::from(heard));
                }
            }
        }
        if self.taking_in {
            self.release_presented(&mut answers);
        }
        if (held || presents > 0) && !self.run.holds.is_empty() {
            host::request_repaint();
        }
        self.apply(answers);
        if let Some(frame) = &self.held_frame
            && !self.waits_for_children(frame.presents())
            && let Some(frame) = self.held_frame.take()
        {
            self.show_frame(frame);
        }
        self.answer();
    }

    /// Moves the held answers into `answers` in order, up to the first children still waiting on
    /// the frame they came with.
    fn release_presented(&mut self, answers: &mut Vec<Answer>) {
        answers.extend(
            self.run
                .holds
                .release_presented()
                .into_iter()
                .map(|held| match held {
                    Held::Laid(laid) => Answer::Laid(laid),
                    Held::Answer(answer) => answer,
                }),
        );
    }

    fn export_previews(&mut self, frame: &Frame) -> bool {
        let shown: Vec<ScreenPlacement> = self
            .run
            .layout()
            .screens
            .iter()
            .filter(|placement| placement.view == ViewId::PREVIEW)
            .copied()
            .collect();
        if shown.is_empty() {
            return false;
        }
        let surfaces: Vec<u32> = shown.iter().map(|placement| placement.surface).collect();
        let painted = Backend::pictures(frame, &surfaces)
            .into_iter()
            .filter_map(|(surface, picture)| {
                let placement = shown
                    .iter()
                    .find(|placement| placement.surface == surface)?;
                Some(previews::Painted {
                    instance: placement.instance,
                    screen: placement.screen,
                    picture,
                    size: [placement.width, placement.height],
                })
            })
            .collect::<Vec<_>>();
        let exported = !painted.is_empty();
        previews::exported(&self.app.identity.id, &self.shared, painted);
        exported
    }

    /// Takes the app's screens. An anchored view they leave out stays, showing its last frame,
    /// until the frame that closed it is drawn (`Shown::place`): what the view showed may have moved
    /// into another of the app's surfaces (a dragged dock bar settling back into its band), which
    /// shows only with that frame, so it is never missing in between, as children come and go
    /// with the frame that fills their space. The app sends a layout ahead of the frame's commits.
    fn place(&mut self, layout: ScreenLayout) {
        let instances = &self.instances;
        let shown = self
            .run
            .place(layout, |screen| instances.is_anchored_screen(screen));
        self.laid_out(shown);
    }

    /// Lets the anchored views a layout closed go (`Release::closed`), once the frame that
    /// follows it has been presented and, where the host composites the app's surfaces, its
    /// pixels have arrived.
    fn close_anchored(&mut self, closed: Option<ScreenLayout>) {
        if let Some(layout) = closed {
            self.laid_out(layout);
            mark(&self.app.identity.id);
        }
    }

    fn laid_out(&mut self, layout: ScreenLayout) {
        let shown: Vec<ScreenPlacement> = layout
            .screens
            .iter()
            .filter(|placement| placement.view == ViewId::PREVIEW)
            .copied()
            .collect();
        previews::keep(&self.app.identity.id, |instance, screen| {
            shown
                .iter()
                .any(|placement| placement.instance == instance && placement.screen == screen)
        });
        self.backend
            .export(shown.iter().map(|placement| placement.surface).collect());
        self.instances.keep_anchored(&layout);
        let instances = &self.instances;
        self.run.lay_out(layout, |placement| {
            placement.view != ViewId::PREVIEW
                && instances.view_request_of(placement.screen).is_some()
        });
        self.release_kept();
    }

    /// Lets go of each kept frame whose view is gone or whose new frame has been drawn, and of
    /// every texture neither the app's layout nor a kept frame holds.
    fn release_kept(&mut self) {
        let instances = &self.instances;
        self.run
            .shown
            .release_kept(self.run.presents.shown(), |screen| {
                instances.view_request_of(screen).is_some()
            });
        self.shared.borrow_mut().kept = self.run.shown.kept_placements();
        presenter::keep(self.surface, &self.run.retained_surfaces());
    }

    /// The placement a screen is drawn from: its kept last frame while it has one, else the
    /// app's layout's.
    fn shown_placement(&self, screen: ScreenId) -> Option<&ScreenPlacement> {
        self.run.placement(screen)
    }

    fn held_screens(&self) -> be_lib_stable_hash::HashSet<ScreenId> {
        self.run
            .holds
            .laid()
            .flat_map(|laid| self.laid_screens(laid))
            .collect()
    }

    fn laid_screens(&self, laid: &Laid) -> be_lib_stable_hash::HashSet<ScreenId> {
        laid.screens(|instance, view| self.instances.screen_id(instance, view))
    }

    fn lay(&mut self, laid: Laid) -> bool {
        let screens = self.laid_screens(&laid);
        let mut changed = self.instances.set_view_sizes(laid.sizes);
        changed |= self.instances.set_windows(laid.windows);
        let asking = laid.children.clone();
        for placements in laid.children {
            changed |= self.instances.set_children(placements);
        }
        self.instances.hear_keyboard_asks(&asking);
        self.instances.laid(laid.serial, &screens);
        changed |= self.report_docked();
        changed |= super::docks::heard(&self.app.identity.id, laid.docks);
        changed
    }

    fn report_docked(&self) -> bool {
        let mut framed: be_lib_stable_hash::HashMap<_, _> =
            self.instances.frame_children().into_iter().collect();
        let mut changed = false;
        for (placer, children) in self.instances.docked_children() {
            let frame = framed.remove(&placer).flatten();
            changed |= match self.instances.on_screen(placer) {
                true => super::docks::placed(placer, &children, frame),
                false => super::docks::offscreen(placer),
            };
        }
        for (placer, frame) in framed {
            changed |= match self.instances.on_screen(placer) {
                true => super::docks::placed(placer, &[], frame),
                false => super::docks::offscreen(placer),
            };
        }
        changed
    }

    fn apply(&mut self, answers: Vec<Answer>) {
        if answers.is_empty() {
            return;
        }
        let mut replies = Vec::new();
        let mut changed = false;
        let heard = answers
            .iter()
            .any(|answer| !matches!(answer, Answer::LaidOut(_) | Answer::Presented(_)));
        for answer in answers {
            changed |= match answer {
                Answer::Layout(layout) => {
                    self.place(layout);
                    true
                }
                Answer::FrameNeeded => {
                    self.run.pacing.need();
                    true
                }
                Answer::MissingCharacters(missing) => {
                    replies.extend(self.fallbacks.answer(&missing).map(AppCall::Fonts));
                    false
                }
                Answer::Shell(mut call) => {
                    self.instances.translate(&mut call, true);
                    if let be_sdk_api::embed::shell::Call::Tell {
                        instance, messages, ..
                    } = &call
                    {
                        for message in messages {
                            if let be_sdk_api::shell_types::ShellMessage::OpenPalette(mode) =
                                message
                                && self.instances.may_open_palette(*instance)
                            {
                                crate::palette::ask(self.id(), *instance, *mode);
                            }
                        }
                    }
                    self.instances.shell_call(call)
                }
                Answer::Paintings(payload) => {
                    self.painted(&payload);
                    true
                }
                Answer::Editor(instance, answer) => {
                    let mut answered = (instance, answer);
                    self.instances.translate(&mut answered, true);
                    self.instances.answered(answered.0, answered.1)
                }
                Answer::Presented(presented) => self.present(presented),
                Answer::LaidOut(commit) => {
                    let changed = self.commit(commit);
                    self.relaid |= changed;
                    changed
                }
                Answer::Laid(laid) => {
                    let changed = self.lay(laid);
                    self.relaid |= changed;
                    changed
                }
                Answer::Blocks(instance, call) => self.instances.blocks_call(instance, call),
                Answer::Watch(instance, open) => self.instances.watch_published(instance, open),
                Answer::Unwatch(instance, open) => {
                    self.instances.unwatch_published(instance, open);
                    false
                }
                Answer::Published(instance, started) => {
                    self.instances.published_call(instance, started)
                }
                Answer::Debug(instance, call) => {
                    let mut asked = (instance, call);
                    self.instances.translate(&mut asked, true);
                    self.instances.debug_call(asked.0, asked.1)
                }
                Answer::Ended(op) => {
                    self.instances.ended(op);
                    false
                }
                Answer::Service(service) => {
                    self.instances.serve(service);
                    false
                }
                Answer::Network(instance, call) => {
                    self.instances.network(instance, call);
                    false
                }
                Answer::Withdrawn(op) => {
                    self.instances.withdraw(op);
                    false
                }
            };
        }
        if heard || changed {
            mark(&self.app.identity.id);
        }
        self.send(replies);
        self.serve();
        self.answer();
        if changed {
            host::request_repaint();
        }
    }

    fn answer(&mut self) {
        let waiting = self.backend.waiting();
        let answers = self.instances.answers(&waiting);
        let accepted = self.instances.take_accepted();
        let backend = &mut self.backend;
        self.instances
            .relay(|replica, frame| backend.relay(replica, frame));
        let finished = self
            .instances
            .stream_fetches(|body, item| backend.stream_item(body, item));
        if !finished.is_empty() {
            self.backend.answer(finished);
        }
        self.backend.answer_watches(answers, accepted);
    }

    fn present(&mut self, presented: PresentCommit) -> bool {
        let PresentCommit {
            serial: _,
            repaint_after_micros,
            animating,
            presented,
            input_waiting,
        } = presented;
        let delay = repaint_after_micros.map(Duration::from_micros);
        self.run
            .pacing
            .presented(self.now(), animating, delay, input_waiting);
        if let Some(delay) = delay {
            host::request_repaint_after(delay);
        }
        let release = self.run.shown.presented(
            presented.as_ref().map(|presented| presented.sequence),
            self.run.presents.shown(),
        );
        if let Some(presented) = presented {
            // The browser's worker shows a present on its canvases as it makes it, so what each
            // surface holds changes now; natively it changes as the frame is shown (`show_frame`).
            #[cfg(target_arch = "wasm32")]
            self.shared.borrow_mut().paint(&presented.painted);
            self.run.presents.report(presented);
        }
        self.close_anchored(release.closed);
        if release.kept {
            self.release_kept();
        }
        self.drawn = self
            .run
            .layout()
            .screens
            .iter()
            .map(|placement| (placement.screen, (placement.width, placement.height)))
            .collect();
        false
    }

    fn commit(&mut self, commit: LayoutCommit) -> bool {
        let LayoutCommit {
            serial,
            sizes,
            reports,
            windows,
            children,
            window_requests,
            docks,
        } = commit;
        self.run.pacing.laid_out();
        for request in &window_requests {
            if let WindowRequest::Open(open) = request {
                self.instances.adopt_window(open, self.run.layout());
            }
        }
        super::request_windows(&self.app.identity.id, window_requests);
        self.instances.adopt_anchored(&reports, self.run.layout());
        let held = self.held_screens();
        let mut changed = self.instances.confirm(serial, &held);
        changed |= self.instances.set_frame_reports(reports);
        changed |= self.lay(Laid::now(serial, sizes, windows, children));
        changed |= self.instances.refresh_anchoring_views();
        changed |= super::docks::heard(&self.app.identity.id, docks);
        changed
    }

    /// Sends the app `messages` without asking it for a frame for them (`send_for_frame` does).
    fn send<T: Into<Outbound>>(&mut self, messages: Vec<T>) {
        if messages.is_empty() || self.error.is_some() {
            return;
        }
        let messages = ready(
            &self.app.identity.id,
            &mut self.instances,
            &mut self.backend,
            messages.into_iter().map(Into::into).collect(),
        );
        let sending = self.run.queue(messages);
        self.backend_send(sending);
    }

    /// Sends the app `messages` it answers with a frame, asking it for one if there are any
    /// (`Run::send_for_frame`).
    fn send_for_frame<T: Into<Outbound>>(&mut self, messages: Vec<T>) {
        let Self {
            app,
            instances,
            backend,
            run,
            error,
            ..
        } = self;
        let messages = messages.into_iter().map(Into::into).collect();
        let sending = run.send_for_frame(messages, |messages| {
            error
                .is_none()
                .then(|| ready(&app.identity.id, instances, backend, messages))
        });
        self.backend_send(sending);
    }

    fn release_reported(&mut self) {
        let messages = self.instances.take_reported();
        self.send(messages);
    }

    fn deliver(&mut self) {
        let sending = self.run.deliverable();
        self.backend_send(sending);
    }

    fn backend_send(&mut self, sending: Vec<Outbound>) {
        if !sending.is_empty() {
            self.backend.send(sending);
        }
    }

    fn flush(&mut self) {
        self.pump();
        let presented = std::mem::take(&mut self.presented) || self.instances.has_mounted();
        if !presented {
            return;
        }
        self.shared
            .borrow_mut()
            .publish(self.run.layout(), self.run.shown.kept_placements(), None);
    }

    fn template(&self, screen: ScreenId) -> Blit {
        let placement = self.shown_placement(screen);
        let zoom = placement
            .and_then(|placement| self.placements.get(&(placement.instance, placement.view)))
            .map_or(1.0, |placed| placed.zoom);
        Blit {
            surface: self.surface,
            status: self.status.clone(),
            shared: Rc::clone(&self.shared),
            screen,
            quad: Quad::upright(Rect::ZERO),
            source: UNIT,
            zoom,
            picture: placement.is_some_and(|placement| placement.view == ViewId::PREVIEW),
            placed: placement
                .map(|placement| [placement.surface, placement.width, placement.height]),
            layer: None,
        }
    }

    fn state(&self) -> String {
        match (&self.error, self.status.get()) {
            (Some(error), _) => error.clone(),
            (None, PresenterState::Presenting) => "presenting".to_owned(),
            (None, PresenterState::Released) => "released".to_owned(),
            (None, PresenterState::Failed(error) | PresenterState::Unsupported(error)) => error,
            (None, PresenterState::Waiting) => self.backend.state().to_owned(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn place_screens(
    blits: &[(u64, Blit)],
    viewport: beui::Vec2,
    pixels_per_point: f32,
    scale: f32,
    background: beui::color::Color32,
) {
    super::web::set_background(background);
    let frame = super::layers::frame(blits, viewport);
    let mut layers = super::web::overlays(&frame, viewport, pixels_per_point, scale);
    let mut placed = super::web::placements(&frame, scale);
    HOST.with(|host| {
        for runtime in host.borrow_mut().runtimes.values_mut() {
            let slot = u64::from(runtime.surface);
            let placements = placed.remove(&slot).unwrap_or_default();
            let retained = runtime.run.retained_surfaces();
            runtime
                .backend
                .place(placements, layers.remove(&slot), &retained);
        }
    });
}

pub fn install(setup: &beui::native::Setup) {
    install_with(presenter::install(setup));
}

/// Installs the runtime with no beui setup: no presenter and no gpu, so an app that asks for
/// the device is told there is none (plans/headless_apps.md; `Headless`).
pub fn install_headless() {
    install_with(Availability(Ok(())));
}

fn install_with(availability: Availability) {
    super::backend::hand_over_device();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = availability;
        for app in host.warming.take().unwrap_or_default() {
            let _ = host.runtime(&app);
        }
    });
    super::backend::discard_prepared();
}

pub fn exiting() {
    let runtimes = HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability::missing();
        host.warming = None;
        std::mem::take(&mut host.runtimes)
    });
    super::backend::discard_prepared();
    for (_, mut runtime) in runtimes {
        runtime.stop();
    }
    presenter::stop();
}

pub fn replace_gpu(setup: &beui::native::Setup) {
    install(setup);
    HOST.with(|host| {
        for runtime in host.borrow_mut().runtimes.values_mut() {
            runtime.restart();
        }
    });
}

pub struct HostFrame {
    insets: Option<be_sdk_api::Insets>,
    pub floating: Vec<Rect>,
}

impl HostFrame {
    pub fn content(&self, region: Rect) -> Rect {
        self.insets.map_or(region, |insets| insets.inside(region))
    }

    pub fn holds_content(&self) -> bool {
        self.insets.is_some()
    }
}

/// The child `child` of `instance` asked for its details from a bar the host
/// does not show; its placer hears it as if from the child's bar.
pub fn report_child_details(app_id: &str, instance: EditorInstanceId, child: be_sdk_api::ChildId) {
    with(app_id, |runtime| {
        runtime.send(vec![Outbound::Event(
            instance,
            EditorEvent::ChildDetails(child),
        )]);
    });
}

pub fn report_child_focus(app_id: &str, instance: EditorInstanceId, child: be_sdk_api::ChildId) {
    with(app_id, |runtime| {
        runtime.send(vec![Outbound::Event(
            instance,
            EditorEvent::ChildFocused(child),
        )]);
    });
}

pub fn preview_image(app_id: &str, instance: EditorInstanceId) -> Option<PreviewImage> {
    previews::image(app_id, instance)
}

pub fn report_children(
    app_id: &str,
    instance: EditorInstanceId,
    region: ViewId,
    statuses: Vec<HostChildStatus>,
) {
    with(app_id, |runtime| {
        let messages = runtime
            .instances
            .set_child_statuses(instance, region, statuses);
        if !messages.is_empty() {
            host::request_repaint();
        }
        runtime.send_for_frame(messages);
    });
}

pub fn picked(app_id: &str, instance: EditorInstanceId) -> Option<BlockPick> {
    with(app_id, |runtime| runtime.instances.picked(instance)).flatten()
}

pub fn card(app_id: &str, instance: EditorInstanceId) -> bool {
    with(app_id, |runtime| runtime.instances.card(instance)).unwrap_or(false)
}

/// The instance's view gives the keyboard it has to `child`, which fills the view.
pub fn pass_child_focus(app_id: &str, instance: EditorInstanceId, child: be_sdk_api::ChildId) {
    if with(app_id, |runtime| {
        runtime.instances.ask_keyboard(instance, child)
    })
    .unwrap_or(false)
    {
        mark(app_id);
    }
}

/// Where the instance's content moved when its view `from` goes: for its main view, the dock tab
/// view that holds it, as a joined editor's content moves there once its dock is hosted. Content
/// moves only that way; a dock tab view that goes leaves nothing to the main view.
pub fn content_moved(app_id: &str, instance: EditorInstanceId, from: ViewId) -> Option<ViewId> {
    if from != ViewId::MAIN {
        return None;
    }
    with(app_id, |runtime| {
        runtime.instances.content_view_besides(instance, Some(from))
    })
    .flatten()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FramePlacement {
    pub rect: Rect,
    pub zoom: f32,
}

pub fn view_placement_of(
    app_id: &str,
    instance: EditorInstanceId,
    region: ViewId,
) -> Option<FramePlacement> {
    with(app_id, |runtime| {
        runtime
            .placements
            .get(&(instance, region))
            .map(|placed| FramePlacement {
                rect: placed.rect,
                zoom: placed.zoom,
            })
    })
    .flatten()
}

fn scale_rect(rect: Rect, size: Vec2) -> Rect {
    Rect::from_min_max(
        pos2(rect.min.x * size.x, rect.min.y * size.y),
        pos2(rect.max.x * size.x, rect.max.y * size.y),
    )
}

pub fn creation(slot: CreationSlot<'_>) -> CreationState {
    let CreationSlot {
        app,
        block_types,
        client_id,
        instance,
        role,
    } = slot;
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let runtime = match host.runtime(app) {
            Ok(runtime) => runtime,
            Err(error) => return CreationState::Failed(error),
        };
        let ready = runtime.contain(|runtime| {
            runtime.error.is_none().then(|| {
                let ready =
                    runtime
                        .instances
                        .report_creation(instance, client_id, block_types, role);
                runtime.release_reported();
                ready
            })
        });
        match (ready, runtime.error.clone()) {
            (_, Some(error)) => CreationState::Failed(error),
            (Some(true), None) => CreationState::Ready,
            (Some(false) | None, None) => CreationState::Starting,
        }
    })
}

/// Starts a host component (the manifest's `hosts`, guides/apps/host_services.md, "Host
/// components") on `block`: an instance with no view, which opens once its app is up and runs
/// until `close`.
pub fn host(
    app: &AppManifest,
    block_types: &Arc<crate::AppCatalog>,
    client_id: Uuid,
    instance: EditorInstanceId,
    host: Uuid,
    block: crate::EditorBlock,
) -> CreationState {
    creation(CreationSlot {
        app,
        block_types,
        client_id,
        instance,
        role: InstanceRole::Host(host, block),
    })
}

pub fn artifact(slot: ArtifactSlot<'_>) -> ArtifactState {
    let ArtifactSlot {
        app,
        block_types,
        client_id,
        instance,
        editor,
        block,
        source,
        exporter,
        data,
        resync,
    } = slot;
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let runtime = match host.runtime(app) {
            Ok(runtime) => runtime,
            Err(error) => return ArtifactState::Failed(error),
        };
        let description = runtime.contain(|runtime| {
            if runtime.error.is_some() {
                return None;
            }
            let messages = runtime.instances.report_artifact(
                instance,
                client_id,
                block_types,
                editor,
                block,
                source,
                exporter,
                data,
                resync,
            );
            runtime.send(messages);
            runtime.release_reported();
            runtime.instances.artifact_description(instance)
        });
        match (description, runtime.error.clone()) {
            (_, Some(error)) => ArtifactState::Failed(error),
            (Some(ArtifactDescription::Described(summary)), None) => {
                ArtifactState::Described(summary)
            }
            (Some(ArtifactDescription::Unreadable(error)), None) => ArtifactState::Failed(error),
            (None, None) => ArtifactState::Starting,
        }
    })
}

thread_local! {
    /// What the shell last said of its phone layout, which every frame spec carries.
    static PHONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Asks to set it, heard while an app's runtime is busy, taken at the next `poll`.
    static PHONE_ASKED: RefCell<Vec<(String, EditorInstanceId, bool)>> =
        const { RefCell::new(Vec::new()) };
}

/// An app says it lays out as on a phone (`set-phone-layout`); only the shell is heard.
pub(crate) fn ask_phone_layout(app_id: &str, instance: EditorInstanceId, on: bool) {
    PHONE_ASKED.with_borrow_mut(|asked| asked.push((app_id.to_owned(), instance, on)));
    host::request_repaint();
}

fn take_phone_asks(host: &mut Host) {
    let asked = PHONE_ASKED.with_borrow_mut(std::mem::take);
    let shell = host.shell.clone();
    let mut changed = false;
    for (app_id, instance, on) in asked {
        if shell.as_ref() == Some(&(app_id, instance)) && PHONE.replace(on) != on {
            changed = true;
        }
    }
    if changed {
        place_every_offer(host);
    }
}

/// Places every view again, so each frame spec carries the phone layout as it now is.
fn place_every_offer(host: &mut Host) {
    for (app_id, runtime) in &mut host.runtimes {
        let placed: Vec<(EditorInstanceId, ViewId)> = runtime.offers.keys().copied().collect();
        for (instance, region) in placed {
            apply_offers(runtime, instance, region);
        }
        mark(app_id);
    }
}

pub fn poll() {
    let pass = host::pass();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        take_phone_asks(&mut host);
        for runtime in host.runtimes.values_mut() {
            runtime.contain(|runtime| {
                if std::mem::take(&mut runtime.restart_asked) {
                    runtime.restart();
                }
                runtime.detect_error();
                runtime.taking_in = true;
                runtime.pump();
                runtime.taking_in = false;
                runtime.begin_frame(pass);
            });
        }
        let grabbed = host
            .runtimes
            .values()
            .any(|runtime| runtime.instances.grabbing());
        if host.grabbed != grabbed {
            host.grabbed = grabbed;
            be_host_base::host::set_grab(grabbed);
        }
        if let Some(wake) = host.stop_idle(host::now()) {
            host::request_repaint_after(wake);
        }
    });
}

/// Restarts the app at the next `poll`, as the person asked.
pub fn restart_app(app_id: &str) {
    if with(app_id, |runtime| runtime.restart_asked = true).is_some() {
        host::request_repaint();
    }
}

pub fn settle() -> bool {
    let pass = host::pass();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let host = &mut *host;
        let origin = host.origin;
        let deadline = host
            .clock
            .deadline(pass, host::clock_stopped(), origin.elapsed());
        let asked: Vec<String> = host
            .runtimes
            .iter_mut()
            .filter_map(|(id, runtime)| {
                runtime.take_relaid();
                runtime
                    .contain(|runtime| runtime.ask(pass))
                    .then(|| id.clone())
            })
            .collect();
        request_frames(&mut host.runtimes, &asked);
        let mut asked = Asked {
            runtimes: &mut host.runtimes,
            asked: &asked,
            origin,
        };
        host.clock.settle(&mut asked, deadline)
    })
}

pub fn begin_frame(
    next_present: Option<Instant>,
    refresh: Option<Duration>,
    prepared: Option<Instant>,
) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let origin = host.origin;
        let since = |at: Instant| at.saturating_duration_since(origin);
        host.clock
            .begin_frame(next_present.map(since), refresh, prepared.map(since));
    });
}

pub fn busy() -> Vec<String> {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let mut reasons: Vec<String> = host
            .runtimes
            .iter_mut()
            .flat_map(|(id, runtime)| {
                runtime
                    .contain(Runtime::busy)
                    .into_iter()
                    .map(move |reason| format!("the app {id} {reason}"))
            })
            .collect();
        reasons.sort();
        reasons
    })
}

pub fn request_painting() {
    each(|runtime| {
        runtime.contain(|runtime| {
            runtime.request_painting();
            Some(())
        });
    });
    host::request_repaint();
}

pub fn drawing() -> bool {
    HOST.with(|host| {
        host.borrow_mut()
            .runtimes
            .values_mut()
            .any(|runtime| runtime.contain(Runtime::drawing))
    })
}

pub fn start_frames(laying_out: bool) {
    let pass = host::pass();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let started: Vec<String> = host
            .runtimes
            .iter_mut()
            .filter_map(|(id, runtime)| {
                runtime
                    .contain(|runtime| runtime.start(pass))
                    .then(|| id.clone())
            })
            .collect();
        if !laying_out {
            request_frames(&mut host.runtimes, &started);
        }
    });
}

pub fn record_pacing() {
    let (waited, over_budget) = HOST.with(|host| host.borrow_mut().clock.take_waited());
    be_host_base::performance::record_group_duration(PACING, "Waiting on apps", waited);
    be_host_base::performance::record_group_count(PACING, "Frames over budget", over_budget);
}

pub fn flush() {
    HOST.with(|host| {
        for runtime in host.borrow_mut().runtimes.values_mut() {
            runtime.contain(Runtime::flush);
        }
    });
}

pub fn set_focus(block: Option<(Uuid, Uuid)>, editor: Option<Uuid>, via: Vec<Uuid>) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let focus = Focus { block, editor, via };
        if host.focus == focus {
            return;
        }
        host.focus = focus.clone();
        for runtime in host.runtimes.values_mut() {
            if runtime.instances.set_focus(focus.clone()) {
                runtime.run.pacing.need();
            }
        }
    });
}

struct Watches;

impl be_host_base::watches::WatchHost for Watches {
    fn published(&self, open: be_sdk_api::ops::MethodId, value: Arc<Vec<u8>>) {
        HOST.with(|host| {
            let mut host = host.borrow_mut();
            for (app_id, runtime) in &mut host.runtimes {
                if runtime.instances.publish(open, Arc::clone(&value)) {
                    runtime.run.pacing.need();
                    mark(app_id);
                    host::request_repaint();
                }
            }
        });
    }

    fn watched(&self, open: be_sdk_api::ops::MethodId) -> bool {
        HOST.with(|host| {
            host.borrow()
                .runtimes
                .values()
                .any(|runtime| runtime.instances.watches_published(open))
        })
    }

    fn take_calls(&self, accept: &mut dyn FnMut(&be_sdk_api::ops::Started) -> bool) {
        HOST.with(|host| {
            let mut host = host.borrow_mut();
            let mut runtimes: Vec<_> = host.runtimes.iter_mut().collect();
            runtimes.sort_by_key(|(left, _)| *left);
            for (_, runtime) in runtimes {
                runtime.instances.take_calls(accept);
            }
        });
    }
}

pub fn set_shell(shell: Option<(&str, EditorInstanceId)>) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let shell = shell.map(|(app_id, instance)| (app_id.to_owned(), instance));
        if host.shell == shell {
            return;
        }
        host.shell = shell.clone();
        // A new shell says again how it lays out; until it does, frames lay out as on a desktop.
        if PHONE.replace(false) {
            place_every_offer(&mut host);
        }
        for (app_id, runtime) in &mut host.runtimes {
            let instance = shell
                .as_ref()
                .filter(|(shell, _)| shell == app_id)
                .map(|(_, instance)| *instance);
            if runtime.instances.set_shell(instance) {
                runtime.run.pacing.need();
                mark(app_id);
                host::request_repaint();
            }
        }
    });
}

pub fn menu(app_id: &str, instance: EditorInstanceId) -> Vec<be_sdk_api::MenuEntry> {
    with(app_id, |runtime| runtime.instances.menu(instance)).unwrap_or_default()
}

pub fn menu_pick(app_id: &str, instance: EditorInstanceId, id: String, anchor: Option<beui::Rect>) {
    with(app_id, |runtime| {
        let messages = runtime.instances.menu_pick(instance, id, anchor);
        runtime.send(messages);
    });
}

pub fn show_dialog(
    app_id: &str,
    instance: EditorInstanceId,
    block: Uuid,
    dialog: be_sdk_api::ShellDialog,
) {
    with(app_id, |runtime| {
        let calls = runtime.instances.show_dialog(instance, block, dialog);
        runtime.send(calls);
    });
}

/// Tells the instance where in its block it was opened, which it selects where it can. Answers
/// whether it could: false until the instance is open.
pub fn reveal(app_id: &str, instance: EditorInstanceId, at: be_sdk_api::SearchLocation) -> bool {
    with(app_id, |runtime| {
        let calls = runtime.instances.reveal(instance, at);
        let told = !calls.is_empty();
        runtime.send(calls);
        told
    })
    .unwrap_or(false)
}

pub fn show_history(
    app_id: &str,
    instance: EditorInstanceId,
    block: uuid::Uuid,
    commit: Option<[u8; 32]>,
) {
    with(app_id, |runtime| {
        runtime.instances.show_history(instance, block, commit);
        runtime.answer();
    });
}

pub fn show_panel(app_id: &str, instance: EditorInstanceId, panel: HostPanel) {
    with(app_id, |runtime| {
        runtime.instances.show_panel(instance, panel);
        runtime.answer();
    });
}

pub fn show_block(app: &AppManifest, instance: EditorInstanceId, open: crate::BlockOpen) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let Ok(runtime) = host.runtime(app) else {
            return;
        };
        runtime.contain(|runtime| {
            runtime.instances.show_block(instance, open);
            runtime.answer();
        });
    });
    mark(&app.identity.id);
}

pub fn set_artifact_states(
    app_id: &str,
    instance: EditorInstanceId,
    states: Vec<be_sdk_api::ArtifactState>,
) {
    with(app_id, |runtime| {
        runtime.instances.set_artifact_states(instance, states);
        runtime.answer();
    });
}

pub fn kill(app_id: &str) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let Some(runtime) = host.runtimes.get_mut(app_id) else {
            return;
        };
        runtime.stopped("It was killed.".to_owned());
        if runtime.instances.is_empty() {
            host.shutdown(app_id);
        }
    });
    mark(app_id);
}

pub fn close(app_id: &str, instance: EditorInstanceId) {
    super::instances::want_presence(instance, false);
    super::docks::forget(instance);
    with(app_id, |runtime| {
        let messages = runtime
            .instances
            .remove(instance)
            .then(|| vec![Outbound::Close(instance)]);
        if let Some(messages) = messages {
            runtime.send(messages);
        }
        runtime.release_kept();
    });
    mark(app_id);
}

/// Why the artifact instance could not make its artifact again as its source last changed.
pub fn artifact_failure(app_id: &str, instance: EditorInstanceId) -> Option<String> {
    with(app_id, |runtime| {
        runtime.instances.artifact_failure(instance)
    })
    .flatten()
}

pub fn artifact_draft(app_id: &str, instance: EditorInstanceId) -> Option<Vec<u8>> {
    with(app_id, |runtime| runtime.instances.artifact_draft(instance)).flatten()
}

pub fn regenerate_artifact(app_id: &str, instance: EditorInstanceId, data: &[u8]) {
    with(app_id, |runtime| {
        let messages = runtime.instances.regenerate_artifact(instance, data);
        runtime.send(messages);
    });
}

pub fn view_size(app_id: &str, instance: EditorInstanceId, region: ViewId) -> Option<Vec2> {
    with(app_id, |runtime| {
        runtime.instances.view_size(instance, region)
    })
    .flatten()
}

pub fn dialog_status(app_id: &str, instance: EditorInstanceId) -> Option<be_sdk_api::DialogStatus> {
    with(app_id, |runtime| runtime.instances.dialog_status(instance)).flatten()
}

pub fn forget_dialog_answer(app_id: &str, instance: EditorInstanceId) {
    with(app_id, |runtime| {
        runtime.instances.forget_dialog_answer(instance)
    });
}

pub fn dismiss(app_id: &str, instance: EditorInstanceId) {
    with(app_id, |runtime| {
        let messages = runtime.instances.dismiss(instance);
        runtime.send(messages);
    });
}

pub fn commit_creation(app_id: &str, instance: EditorInstanceId) {
    with(app_id, |runtime| {
        let messages = runtime.instances.commit_creation(instance);
        runtime.send(messages);
    });
}

/// The child that has taken `instance`'s frame: the view placing it, the child and its block.
pub fn frame_child(
    app_id: &str,
    instance: EditorInstanceId,
) -> Option<(ViewId, be_sdk_api::ChildId, Uuid)> {
    with(app_id, |runtime| runtime.instances.frame_child(instance)).flatten()
}

pub fn revoke_frame_child(app_id: &str, instance: EditorInstanceId) {
    with(app_id, |runtime| {
        runtime.instances.revoke_frame_child(instance);
        runtime.report_docked();
    });
    mark(app_id);
}

pub fn hold(app_id: &str, instance: EditorInstanceId, region: ViewId) {
    with(app_id, |runtime| runtime.instances.hold(instance, region));
}

pub fn frame_rects(app_id: &str, instance: EditorInstanceId, view: ViewId) -> Option<HostFrame> {
    with(app_id, |runtime| {
        runtime
            .instances
            .frame_report(instance, view)
            .map(|report| HostFrame {
                insets: report.content,
                floating: report.floating.clone(),
            })
    })
    .flatten()
}

/// For a docked instance: the app and instance placing it, and its child id there.
pub fn docked_placer(
    instance: EditorInstanceId,
) -> Option<(String, EditorInstanceId, be_sdk_api::ChildId)> {
    crate::docks::placed_by(instance)
}

pub fn content_view(app_id: &str, instance: EditorInstanceId) -> ViewId {
    with(app_id, |runtime| runtime.instances.content_view(instance)).unwrap_or(ViewId::MAIN)
}

pub fn presenting(app_id: &str, instance: EditorInstanceId) -> bool {
    with(app_id, |runtime| runtime.instances.presenting(instance)).unwrap_or_default()
}

pub fn present(app_id: &str, instance: EditorInstanceId, presenting: bool) {
    with(app_id, |runtime| {
        if runtime.instances.set_presenting(instance, presenting) {
            mark(app_id);
            host::request_repaint();
        }
    });
}

pub fn set_presented(app_id: &str, instance: EditorInstanceId, presented: bool) {
    with(app_id, |runtime| {
        if runtime.instances.set_presented(instance, presented) {
            runtime.run.pacing.need();
            mark(app_id);
            host::request_repaint();
        }
    });
}

pub fn set_presence_visible(app_id: &str, instance: EditorInstanceId, visible: bool) {
    super::instances::want_presence(instance, visible);
    with(app_id, |runtime| {
        let messages = runtime.instances.set_presence_visible(instance, visible);
        runtime.send(messages);
    });
}

/// Asks an instance to render a representation of its copy `serial`; an
/// `InstanceEvent::Rendered` with the request it answers answers it. `None` when
/// the instance is gone.
pub fn render_clip(
    app_id: &str,
    instance: EditorInstanceId,
    serial: u64,
    format: String,
) -> Option<u64> {
    with(app_id, |runtime| {
        let (messages, request) = runtime.instances.render_clip(instance, serial, format);
        runtime.send(messages);
        request
    })
    .flatten()
}

pub fn resized(app_id: &str, instance: EditorInstanceId, size: Vec2) {
    with(app_id, |runtime| {
        let messages = runtime.instances.resized(instance, size);
        runtime.send(messages);
    });
}

pub fn assigned_size(app_id: &str, instance: EditorInstanceId) -> Option<Vec2> {
    with(app_id, |runtime| runtime.instances.assigned_size(instance)).flatten()
}

pub fn intrinsic_size(app_id: &str, instance: EditorInstanceId) -> Option<Vec2> {
    with(app_id, |runtime| runtime.instances.intrinsic_size(instance)).flatten()
}

pub fn warm(app: &AppManifest) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let Some(warming) = &mut host.warming else {
            let _ = host.runtime(app);
            return;
        };
        if !warming
            .iter()
            .any(|warmed| warmed.identity.id == app.identity.id)
        {
            warming.push(app.clone());
            super::backend::prepare(app);
        }
    });
}

pub fn in_use() -> Vec<String> {
    HOST.with(|host| {
        let mut in_use: Vec<String> = host
            .borrow()
            .runtimes
            .iter()
            .filter(|(_, runtime)| !runtime.instances.is_empty())
            .map(|(app_id, _)| app_id.clone())
            .collect();
        in_use.sort_unstable();
        in_use
    })
}

/// What hosted blocks need of the runtime (plans/hosted_blocks.md, "Hosting"): each block an
/// editor shows, with its type, and each host component running, by its app, instance and
/// block.
pub fn editing_and_hosting() -> (Vec<EditorBlock>, Vec<(String, EditorInstanceId, Uuid)>) {
    HOST.with(|host| {
        let host = host.borrow();
        let mut editing = Vec::new();
        let mut hosting = Vec::new();
        for (app_id, runtime) in &host.runtimes {
            for (instance, role) in runtime.instances.roles() {
                match role {
                    InstanceRole::Editor(_, block) => editing.push(block),
                    InstanceRole::Host(_, block) => {
                        hosting.push((app_id.clone(), instance, block.id));
                    }
                    _ => {}
                }
            }
        }
        (editing, hosting)
    })
}

pub fn running() -> Vec<RuntimeStatus> {
    HOST.with(|host| {
        let host = host.borrow();
        let mut running: Vec<_> = host
            .runtimes
            .iter()
            .map(|(app_id, runtime)| RuntimeStatus {
                app_id: app_id.clone(),
                state: runtime.state(),
                surface: SurfaceStatus {
                    index: runtime.surface,
                    serial: runtime.instances.serial(),
                    pixels: runtime
                        .run
                        .layout()
                        .screens
                        .iter()
                        .map(|placement| u64::from(placement.width) * u64::from(placement.height))
                        .sum(),
                    placements: runtime.run.layout().screens.len(),
                },
                steps: runtime.steps,
                frames: runtime.frames,
                uptime: runtime.backend.uptime(),
                instances: runtime
                    .instances
                    .statuses(runtime.run.layout(), runtime.pass),
            })
            .collect();
        running.sort_by(|left, right| left.app_id.cmp(&right.app_id));
        running
    })
}

fn session() -> Session {
    let surface = SurfaceSpec {
        format: SurfaceFormat::Rgba8Unorm,
        render: presenter::rendered(),
    };
    Session::new(HOST_NAME, Some(surface))
}

thread_local! {
    static CHANGED: RefCell<be_lib_stable_hash::HashSet<String>> =
        RefCell::new(be_lib_stable_hash::HashSet::default());
    static STOPPED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn heard_stop(app_id: &str) {
    STOPPED.with(|stopped| stopped.borrow_mut().push(app_id.to_owned()));
}

pub fn take_stopped() -> Vec<String> {
    STOPPED.with(|stopped| std::mem::take(&mut *stopped.borrow_mut()))
}

pub fn crash(app_id: &str, error: String) {
    with(app_id, |runtime| runtime.crashed(error));
}

pub(super) fn mark(app_id: &str) {
    CHANGED.with(|changed| {
        if !changed.borrow().contains(app_id) {
            changed.borrow_mut().insert(app_id.to_owned());
        }
    });
}

pub fn take_changed() -> Vec<String> {
    CHANGED.with(|changed| changed.borrow_mut().drain().collect())
}

#[derive(Clone, Copy, Debug)]
pub struct ViewPlacer<'a> {
    pub app: &'a str,
    pub instance: EditorInstanceId,
    pub view: ViewId,
}

#[derive(Clone, Debug, PartialEq)]
struct Placer {
    app: String,
    instance: EditorInstanceId,
    view: ViewId,
}

pub struct ViewSlot<'a> {
    pub app: &'a AppManifest,
    pub placer: Option<ViewPlacer<'a>>,
    pub block_types: &'a Arc<crate::AppCatalog>,
    pub client_id: Uuid,
    pub role: InstanceRole,
    pub instance: EditorInstanceId,
    pub view: View,
}

pub fn mount_view(slot: ViewSlot<'_>) {
    let ViewSlot {
        app,
        placer,
        block_types,
        client_id,
        role,
        instance,
        view,
    } = slot;
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let Ok(runtime) = host.runtime(app) else {
            mark(&app.identity.id);
            return;
        };
        runtime.contain(|runtime| {
            super::docks::register(
                instance,
                &app.identity.id,
                match role {
                    InstanceRole::Editor(_, block) => Some(block.id),
                    InstanceRole::Creation(..)
                    | InstanceRole::Artifact(..)
                    | InstanceRole::Picker(_)
                    | InstanceRole::Rename(..)
                    | InstanceRole::Export(..)
                    | InstanceRole::Host(..) => None,
                },
            );
            runtime
                .instances
                .mount(instance, view, client_id, role, block_types);
            match placer {
                Some(placer) => runtime.placers.insert(
                    (instance, view.id),
                    Placer {
                        app: placer.app.to_owned(),
                        instance: placer.instance,
                        view: placer.view,
                    },
                ),
                None => runtime.placers.remove(&(instance, view.id)),
            };
            runtime.release_reported();
        });
        mark(&app.identity.id);
    });
    host::request_repaint();
}

pub fn unmount_view(app_id: &str, instance: EditorInstanceId, region: ViewId, mount: u64) {
    with(app_id, |runtime| {
        runtime.instances.unmount(instance, region);
        if !runtime.instances.mounted(instance, region) {
            runtime.placers.remove(&(instance, region));
        }
        withdraw(runtime, instance, region, mount);
        runtime.report_docked();
    });
    mark(app_id);
    host::request_repaint();
}

pub fn unplace_view(app_id: &str, instance: EditorInstanceId, region: ViewId, mount: u64) {
    with(app_id, |runtime| {
        withdraw(runtime, instance, region, mount);
        runtime.report_docked();
    });
    mark(app_id);
    host::request_repaint();
}

pub fn set_view_inert(
    app_id: &str,
    instance: EditorInstanceId,
    region: ViewId,
    mount: u64,
    inert: bool,
) {
    with(app_id, |runtime| {
        let key = (instance, region);
        let Some(offers) = runtime.offers.get_mut(&key) else {
            return;
        };
        if let Some(offer) = offers.iter_mut().find(|offer| offer.mount == mount) {
            offer.inert = inert;
        }
        let inert = offers.iter().all(|offer| offer.inert);
        if let Some(placed) = runtime.placements.get_mut(&key) {
            placed.inert = inert;
        }
    });
    host::request_repaint();
}

fn withdraw(runtime: &mut Runtime, instance: EditorInstanceId, region: ViewId, mount: u64) {
    let key = (instance, region);
    let remaining = runtime.offers.get_mut(&key).map_or(0, |offers| {
        offers.retain(|offer| offer.mount != mount);
        offers.len()
    });
    match remaining {
        0 => {
            runtime.offers.remove(&key);
            runtime.placements.remove(&key);
            runtime.instances.unplace(instance, region);
        }
        _ => apply_offers(runtime, instance, region),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewPlace {
    pub rect: Rect,
    pub clip: Rect,
    pub zoom: f32,
    pub raster: f32,
    pub windows: bool,
    pub placement: Option<String>,
    pub origin: Option<Pos2>,
    pub anchor_bounds: Option<Rect>,
    pub fit: Option<Vec2>,
}

pub(super) struct Offer {
    mount: u64,
    placement: ViewPlace,
    frame: Option<FrameSpec>,
    view: Option<EditorView>,
    inert: bool,
}

pub fn place_view(
    app_id: &str,
    instance: EditorInstanceId,
    region: ViewId,
    mount: u64,
    placement: ViewPlace,
    frame: Option<FrameSpec>,
    view: Option<EditorView>,
) {
    with(app_id, |runtime| {
        if !runtime.instances.mounted(instance, region) {
            return;
        }
        let offers = runtime.offers.entry((instance, region)).or_default();
        let inert = offers
            .iter()
            .find(|held| held.mount == mount)
            .is_some_and(|held| held.inert);
        let offer = Offer {
            mount,
            placement,
            frame,
            view,
            inert,
        };
        match offers.iter_mut().find(|held| held.mount == mount) {
            Some(held) => *held = offer,
            None => offers.push(offer),
        }
        apply_offers(runtime, instance, region);
        mark(app_id);
    });
    host::request_repaint();
}

fn apply_offers(runtime: &mut Runtime, instance: EditorInstanceId, region: ViewId) {
    let Some(offers) = runtime.offers.get(&(instance, region)) else {
        return;
    };
    let area = |offer: &Offer| offer.placement.rect.width() * offer.placement.rect.height();
    let Some(chosen) = offers.iter().max_by(|a, b| area(a).total_cmp(&area(b))) else {
        return;
    };
    let shown = match region {
        ViewId::PREVIEW => Some(UNIT),
        _ => offers
            .iter()
            .filter_map(|offer| presenter::shown_part(offer.placement.rect, offer.placement.clip))
            .reduce(|shown, source| shown.union(source)),
    };
    let seen = |offer: &Offer| {
        let shown = offer.placement.rect.intersect(offer.placement.clip);
        shown.width().max(0.0) * shown.height().max(0.0)
    };
    let raster = offers
        .iter()
        .filter(|offer| offer.placement.raster.is_finite() && offer.placement.raster > 0.0)
        .max_by(|a, b| seen(a).total_cmp(&seen(b)))
        .map(|offer| offer.placement.raster);
    let windows = chosen.placement.windows;
    let host_placement = chosen.placement.placement.clone();
    let origin = chosen.placement.origin;
    let anchor_bounds = chosen.placement.anchor_bounds;
    let fit = chosen
        .placement
        .fit
        .filter(|fit| region != ViewId::PREVIEW && fit.x > 0.0 && fit.y > 0.0);
    let inert = offers.iter().all(|offer| offer.inert);
    let frame = chosen.frame.clone().map(|frame| FrameSpec {
        phone: PHONE.get(),
        ..frame
    });
    let view = chosen.view.clone();
    let ViewPlace {
        rect, clip, zoom, ..
    } = chosen.placement;
    runtime.placed = host::pass();
    let zoom = match zoom.is_finite() && zoom > 0.0 && region != ViewId::PREVIEW {
        true => zoom,
        false => 1.0,
    };
    runtime.placements.insert(
        (instance, region),
        Placed {
            rect,
            clip,
            zoom,
            inert,
            shown: shown.unwrap_or(Rect::ZERO),
        },
    );
    let (size, scale_factor) = match (region, fit) {
        (ViewId::PREVIEW, _) => (
            rect.size(),
            preview_scale(rect.size(), host::pixels_per_point()),
        ),
        (_, Some(fit)) => (fit / zoom, host::pixels_per_point() * zoom),
        (_, None) => (rect.size() / zoom, host::pixels_per_point() * zoom),
    };
    let raster = match (raster, region) {
        (None, _) => scale_factor,
        (Some(raster), ViewId::PREVIEW) => preview_scale(rect.size(), scale_factor * raster),
        (Some(raster), _) => scale_factor * raster,
    };
    let visible = match fit {
        Some(_) => Rect::from_min_size(Pos2::ZERO, size),
        None => shown.map_or(Rect::ZERO, |source| scale_rect(source, size)),
    };
    let anchor_space = anchor_bounds
        .filter(|_| region != ViewId::PREVIEW)
        .map(|bounds| AnchorSpace {
            to_host: HostTransform {
                origin: rect.min,
                scale: vec2(
                    ratio_of(rect.width(), size.x),
                    ratio_of(rect.height(), size.y),
                ),
            },
            bounds,
        });
    runtime.instances.place_mounted(
        instance,
        region,
        frame,
        size,
        visible,
        scale_factor,
        raster,
        windows,
        host_placement,
        origin,
        anchor_space,
        fit.is_some(),
    );
    if let Some(view) = view {
        runtime.instances.set_view(instance, view);
    }
    runtime
        .instances
        .place(instance, region, Placement { rect, clip });
}

fn ratio_of(shown: f32, logical: f32) -> f32 {
    match logical > 0.0 && (shown / logical).is_finite() {
        true => shown / logical,
        false => 1.0,
    }
}

pub fn forward_anchored(
    app_id: &str,
    instance: EditorInstanceId,
    region: ViewId,
    layer: ViewId,
    input: &beui::ForwardedInput,
) {
    with(app_id, |runtime| {
        runtime.refresh_anchor_space(instance, region);
        let messages = runtime
            .instances
            .forward_anchored(instance, region, layer, input);
        runtime.send_for_frame(messages);
    });
    host::request_repaint();
}

pub fn dismiss_anchored_views(
    app_id: &str,
    instance: EditorInstanceId,
    region: ViewId,
    dismissed: Vec<ViewId>,
) {
    with(app_id, |runtime| {
        let messages = runtime
            .instances
            .dismiss_anchored_views(instance, region, dismissed);
        runtime.send_for_frame(messages);
    });
    host::request_repaint();
}

/// Forwards the input a view heard to its app. Whether the app was sent anything: if so it will
/// draw a frame that answers it. An app that does not run yet hears only where the keyboard is
/// (`Input::Focus`), state it is told once it runs: every key, press and move is dropped, never
/// held for when it runs (decisions/keys_are_never_held.md).
/// What an app that does not run yet hears of the input routed to its views: only where the
/// keyboard is (`Input::Focus`), which is state; every key, press and move is dropped, never
/// queued for when it runs (decisions/keys_are_never_held.md).
fn keep_only_focus(messages: &mut Vec<Outbound>) {
    for message in messages.iter_mut() {
        if let Outbound::App(AppCall::Input(batch)) = message {
            batch
                .events
                .retain(|event| matches!(event.input, beui::Input::Focus(_)));
        }
    }
    messages.retain(|message| {
        !matches!(message, Outbound::App(AppCall::Input(batch)) if batch.events.is_empty())
    });
}

pub fn forward_view(
    app_id: &str,
    instance: EditorInstanceId,
    region: ViewId,
    input: &beui::ForwardedInput,
    aim: &dyn Fn(Option<beui::Over>, beui::Pos2) -> beui::Aim,
    target: &dyn Fn(beui::NodeId) -> Option<(EditorInstanceId, ViewId)>,
) -> bool {
    with(app_id, |runtime| {
        runtime.refresh_anchor_space(instance, region);
        let (mut messages, revoked) = runtime
            .instances
            .forward(instance, region, input, aim, target);
        if !runtime.run.session.running() {
            keep_only_focus(&mut messages);
        }
        let sent = !messages.is_empty();
        runtime.send_for_frame(messages);
        if revoked {
            mark(app_id);
        }
        sent
    })
    .unwrap_or(false)
}

pub fn pressed_outside_view(app_id: &str, instance: EditorInstanceId, region: ViewId) {
    with(app_id, |runtime| {
        if runtime.instances.revoke_active(instance, region) {
            mark(app_id);
        }
    });
}

pub fn back_view(
    app_id: &str,
    instance: EditorInstanceId,
    region: ViewId,
    gesture: beui::BackGesture,
) {
    with(app_id, |runtime| {
        let messages = runtime.instances.back(instance, region, gesture);
        runtime.send_for_frame(messages);
    });
    host::request_repaint();
}

pub fn intercept_view(
    app_id: &str,
    instance: EditorInstanceId,
    region: ViewId,
    press: Option<beui::KeyPress>,
    modifiers: beui::Modifiers,
) {
    with(app_id, |runtime| {
        let messages = runtime
            .instances
            .intercepted(instance, region, press, modifiers);
        runtime.send_for_frame(messages);
    });
    host::request_repaint();
}

pub fn view_has_keyboard(app_id: &str, instance: EditorInstanceId, region: ViewId) -> bool {
    with(app_id, |runtime| {
        runtime.instances.has_keyboard(instance, region)
    })
    .unwrap_or(false)
}

pub fn intercept_tap(app_id: &str, instance: EditorInstanceId, region: ViewId, key: beui::Key) {
    with(app_id, |runtime| {
        let messages = runtime.instances.intercepted_tap(instance, region, key);
        runtime.send_for_frame(messages);
    });
    host::request_repaint();
}

#[derive(Clone, PartialEq)]
pub struct PlacedView {
    pub error: Option<(String, bool)>,
    pub rect: Rect,
    pub clip: Rect,
    pub loading: bool,
    pub base: Vec<Piece>,
    /// The overlays' pieces, drawn above the view's children and the windows it opens.
    pub floating: Vec<Piece>,
    /// The marks' pieces (`frame-report.marks-painted`), above the view's children and
    /// under the windows it opens.
    pub marks: Vec<Piece>,
    /// Where the overlays and marks take input, over the view's children.
    pub floating_rects: Vec<Rect>,
    /// Where only the overlays take input, over the windows the view opens too.
    pub overlay_rects: Vec<Rect>,
    pub scrims: Vec<beui::layer::Scrim>,
    pub drawing: Vec<Rect>,
    pub claims: Vec<(beui::Modifiers, Rect)>,
    pub held: Option<Rect>,
    pub drawn: Option<(u32, u32)>,
    pub children: Vec<HostChild>,
    pub anchored: Vec<HostAnchored>,
    pub blocks_owner: bool,
    pub holes: Vec<(Rect, Vec<Rect>)>,
    pub cursor: Option<beui::CursorIcon>,
    pub ime: Option<beui::ImeArea>,
    pub handles_back: bool,
    pub wants_keyboard: bool,
    pub keyboard_toggles: Option<u32>,
    pub intercepted_keys: Vec<beui::KeyChord>,
    pub pastes: Vec<String>,
    pub clipboard_keys: bool,
    pub every_sample: bool,
    pub work_area: Option<Rect>,
    pub used: Option<Vec2>,
    /// The opaque color the app clears the view to, which the host fills the view with under
    /// its children (`FrameReport::backdrop`). Until the app reports a frame of the view (a
    /// view shown again, as a dock tab switched back to), the color its instance last reported
    /// for another view, so the space is never left empty while the app draws it.
    pub backdrop: Option<beui::color::Color32>,
    /// Whether the app has reported a frame of the view yet.
    pub reported: bool,
}

impl PlacedView {
    fn failed(error: String, restart: bool) -> Self {
        Self {
            error: Some((error, restart)),
            rect: Rect::ZERO,
            clip: Rect::ZERO,
            loading: false,
            base: Vec::new(),
            floating: Vec::new(),
            marks: Vec::new(),
            floating_rects: Vec::new(),
            overlay_rects: Vec::new(),
            scrims: Vec::new(),
            drawing: Vec::new(),
            claims: Vec::new(),
            held: None,
            drawn: None,
            children: Vec::new(),
            anchored: Vec::new(),
            blocks_owner: false,
            holes: Vec::new(),
            cursor: None,
            ime: None,
            handles_back: false,
            wants_keyboard: false,
            keyboard_toggles: None,
            intercepted_keys: Vec::new(),
            pastes: Vec::new(),
            clipboard_keys: false,
            every_sample: false,
            work_area: None,
            used: None,
            backdrop: None,
            reported: false,
        }
    }

    fn waiting() -> Self {
        Self {
            error: None,
            loading: true,
            ..Self::failed(String::new(), false)
        }
    }

    pub fn takes(&self, local: Pos2) -> bool {
        let position = local + self.rect.min.to_vec2();
        !self.holes.iter().any(|(hole, occluders)| {
            hole.contains(position) && !occluders.iter().any(|rect| rect.contains(position))
        })
    }
}

fn pieces_of(rect: Rect, visible: Rect, textured: Rect, shown: &[Rect]) -> Vec<Piece> {
    let fraction = |at: Rect, of: Rect| {
        Rect::from_min_max(
            pos2(
                (at.min.x - of.min.x) / of.width().max(f32::EPSILON),
                (at.min.y - of.min.y) / of.height().max(f32::EPSILON),
            ),
            pos2(
                (at.max.x - of.min.x) / of.width().max(f32::EPSILON),
                (at.max.y - of.min.y) / of.height().max(f32::EPSILON),
            ),
        )
    };
    shown
        .iter()
        .map(|piece| piece.intersect(visible).intersect(textured))
        .filter(|piece| piece.is_positive())
        .map(|piece| Piece {
            local: fraction(piece, rect),
            source: fraction(piece, textured),
        })
        .collect()
}

pub fn placed_view(
    app: &AppManifest,
    instance: EditorInstanceId,
    region: ViewId,
    own: Option<EmbedPlacement>,
    view: Option<EditorView>,
) -> PlacedView {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let runtime = match host.runtime(app) {
            Ok(runtime) => runtime,
            Err(error) => return PlacedView::failed(error, false),
        };
        let view = runtime.contain(|runtime| {
            runtime
                .error
                .is_none()
                .then(|| shown_view(runtime, instance, region, own, view))
        });
        let placed = own.map(|own| (own.rect, own.clip)).or_else(|| {
            let placement = runtime.instances.placement(instance, region)?;
            Some((placement.rect, placement.clip))
        });
        let unshown = |view: PlacedView| match (view.rect == Rect::ZERO, placed) {
            (true, Some((rect, clip))) => PlacedView { rect, clip, ..view },
            _ => view,
        };
        match (view, runtime.error.clone()) {
            (_, Some(error)) => unshown(PlacedView::failed(error, true)),
            (Some(view), None) => unshown(view),
            (None, None) => unshown(PlacedView::waiting()),
        }
    })
}

fn shown_view(
    runtime: &mut Runtime,
    instance: EditorInstanceId,
    region: ViewId,
    own: Option<EmbedPlacement>,
    view: Option<EditorView>,
) -> PlacedView {
    {
        if let Some(stopped) = runtime.instances.stopped(instance, region) {
            let error = match stopped {
                be_sdk_api::Stopped::Closed => "The editor closed".to_owned(),
                be_sdk_api::Stopped::Failed(failure) => {
                    format!("The editor stopped after a panic: {failure}")
                }
            };
            return PlacedView::failed(error, true);
        }
        let Some(placement) = runtime.instances.placement(instance, region) else {
            return PlacedView::waiting();
        };
        let chosen = (placement.rect, placement.clip);
        let (rect, clip) = own.map_or(chosen, |own| (own.rect, own.clip));
        let screen = runtime.instances.screen_id(instance, region);
        let painted = screen.and_then(|screen| runtime.shown_placement(screen));
        let visible = rect.intersect(clip);
        let shown = runtime
            .placements
            .get(&(instance, region))
            .map_or(UNIT, |placed| placed.shown);
        let textured = Rect::from_min_max(
            rect.min + vec2(shown.min.x * rect.width(), shown.min.y * rect.height()),
            rect.min + vec2(shown.max.x * rect.width(), shown.max.y * rect.height()),
        );
        let held = match (rect, clip) == chosen {
            true => runtime.instances.held(
                instance,
                region,
                Some(visible),
                clip,
                painted.map(|placement| {
                    (
                        (placement.width, placement.height),
                        super::input::finer(placement),
                    )
                }),
            ),
            false => None,
        };
        let (children, holes) = runtime
            .instances
            .host_children(instance, region, rect, clip, view);
        let report = runtime.instances.frame_report(instance, region);
        let in_region = |rects: fn(&be_sdk_api::FrameReport) -> &Vec<Rect>| -> Vec<Rect> {
            match held {
                Some(_) => Vec::new(),
                None => report
                    .map(|report| {
                        rects(report)
                            .iter()
                            .map(|floating| floating.translate(rect.min.to_vec2()))
                            .collect()
                    })
                    .unwrap_or_default(),
            }
        };
        let floating_rects = in_region(|report| &report.floating);
        let painted_rects = in_region(|report| &report.floating_painted);
        let mark_rects = in_region(|report| &report.marks);
        let marks_painted = in_region(|report| &report.marks_painted);
        let overlay_rects: Vec<Rect> = floating_rects
            .iter()
            .filter(|rect| !mark_rects.contains(rect))
            .copied()
            .collect();
        let overlays_painted: Vec<Rect> = painted_rects
            .iter()
            .filter(|rect| !marks_painted.contains(rect))
            .copied()
            .collect();
        let scrims = match held {
            Some(_) => Vec::new(),
            None => report
                .map(|report| {
                    report
                        .scrims
                        .iter()
                        .map(|scrim| beui::layer::Scrim {
                            rect: scrim.rect.translate(rect.min.to_vec2()),
                            color: reported_color(&scrim.color),
                            radius: scrim.radius,
                        })
                        .collect()
                })
                .unwrap_or_default(),
        };
        let window = runtime.instances.window(instance, region);
        let handles_back = window.is_some_and(|window| window.handles_back);
        let claims = report
            .map(|report| {
                report
                    .claims
                    .iter()
                    .map(|claim| {
                        let at = claim.rect.translate(rect.min.to_vec2());
                        (claim.modifiers, at.intersect(visible))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let drawing = report
            .map(|report| {
                report
                    .drawing
                    .iter()
                    .map(|drawing| drawing.translate(rect.min.to_vec2()).intersect(visible))
                    .collect()
            })
            .unwrap_or_default();
        let intercepted_keys = window
            .iter()
            .flat_map(|window| window.intercepted_keys.iter().copied())
            .collect();
        let base: Vec<Rect> = match painted_rects.is_empty() {
            true => vec![visible],
            false => visible.subtract(&painted_rects),
        };
        let (base, floating, marks) = match held {
            Some(_) => (
                vec![Piece {
                    local: UNIT,
                    source: UNIT,
                }],
                Vec::new(),
                Vec::new(),
            ),
            None => (
                pieces_of(rect, visible, textured, &base),
                pieces_of(rect, visible, textured, &overlays_painted),
                pieces_of(rect, visible, textured, &marks_painted),
            ),
        };
        let loading = screen.is_none_or(|screen| runtime.shown_placement(screen).is_none());
        let anchored = runtime.instances.shown_anchored(instance, region);
        let gives_keyboard = |layer: &HostAnchored| {
            layer.keyboard
                && children
                    .iter()
                    .any(|child| child.anchored == Some(layer.id))
        };
        let child_keyboard = anchored.iter().any(gives_keyboard);
        let anchored_keyboard = anchored
            .iter()
            .any(|layer| layer.keyboard && !gives_keyboard(layer));
        PlacedView {
            error: None,
            rect,
            clip,
            loading,
            base,
            floating,
            marks,
            floating_rects,
            overlay_rects,
            scrims,
            drawing,
            claims,
            held: held.map(|held| held.rect),
            drawn: held.map(|held| held.drawn),
            children,
            blocks_owner: anchored.iter().any(|view| view.blocks_owner),
            holes: holes.parts(),
            cursor: match runtime.instances.dragging(instance, region)
                && runtime.instances.drag_accepted(instance)
            {
                true => Some(beui::CursorIcon::Alias),
                false => runtime.instances.cursor(instance, region),
            },
            ime: runtime.instances.ime(instance, region, rect),
            handles_back,
            wants_keyboard: (window.is_some_and(|window| window.wants_keyboard) && !child_keyboard)
                || anchored_keyboard,
            anchored,
            keyboard_toggles: window.map(|window| window.keyboard_toggles),
            intercepted_keys,
            pastes: window
                .map(|window| window.pastes.clone())
                .unwrap_or_default(),
            clipboard_keys: window.is_some_and(|window| window.clipboard_keys),
            every_sample: window.is_some_and(|window| window.every_sample),
            work_area: report
                .and_then(|report| report.work_area)
                .map(|insets| insets.inside(Rect::from_min_size(Pos2::ZERO, rect.size()))),
            used: runtime.instances.view_size(instance, region),
            backdrop: match report {
                Some(report) => report.backdrop.as_ref().map(reported_color),
                None => runtime.instances.backdrop(instance).map(reported_color),
            },
            reported: report.is_some(),
        }
    }
}

pub fn frames(app_id: &str) -> u64 {
    with(app_id, |runtime| runtime.frames).unwrap_or_default()
}

pub fn drag_over_view(
    app_id: &str,
    instance: EditorInstanceId,
    region: ViewId,
    over: Option<(BlockDrag, Pos2, be_sdk_api::DropAction)>,
    dropped: bool,
) {
    with(app_id, |runtime| {
        let origin = runtime
            .instances
            .placement(instance, region)
            .map_or(Pos2::ZERO, |placement| placement.rect.min);
        let was = runtime.instances.dragging(instance, region);
        let event = over.map(|(drag, at, action)| {
            let blocks = be_sdk_api::clipboard::Blocks {
                blocks: vec![be_sdk_api::clipboard::ClipBlock {
                    id: drag.block_id,
                    organization: None,
                    block_type: drag.block_type,
                    title: String::new(),
                }],
            };
            DragEvent {
                position: Pos2::ZERO + (at - origin),
                items: vec![be_sdk_api::clipboard::ClipFormat::encode(&blocks)],
                action,
                dropped,
            }
        });
        let messages = runtime
            .instances
            .drag(instance, region, DragKind::Block, event);
        runtime.send_for_frame(messages);
        if was != runtime.instances.dragging(instance, region) {
            mark(app_id);
        }
    });
}

pub fn view_drawing(
    app_id: &str,
    instance: EditorInstanceId,
    region: ViewId,
    view: &PlacedView,
    pieces: &[Piece],
    rotation: f32,
    opacity: f32,
    corner_radius: f32,
) -> Option<(beui::native::Drawing, super::presenter::Pieces)> {
    if view.loading {
        return None;
    }
    with(app_id, |runtime| {
        let screen = runtime.instances.screen_id(instance, region)?;
        let mut template = runtime.template(screen);
        template.quad.opacity = opacity;
        template.quad.corner_radius = corner_radius;
        let shared = Rc::new(RefCell::new(pieces.to_vec()));
        let drawing = beui::native::painted_drawing(ViewDrawing::new(
            template,
            Rc::clone(&shared),
            view.held,
            rotation,
        ));
        Some((drawing, shared))
    })
    .flatten()
}

pub fn anchored_blits(app_id: &str, layer: &HostAnchored) -> Vec<Blit> {
    with(app_id, |runtime| {
        let template = runtime.template(layer.screen);
        let pixels_per_point = layer.pixels_per_point;
        Some(
            layer
                .painted
                .iter()
                .map(|piece| piece.intersect(layer.rect))
                .filter(|piece| piece.is_positive())
                .map(|piece| {
                    let from = piece.min - layer.rect.min;
                    let to = piece.max - layer.rect.min;
                    Blit {
                        quad: Quad::upright(piece),
                        source: UNIT,
                        layer: Some(AnchoredSource {
                            surface: layer.surface,
                            bounds: layer.bounds,
                            pixels: Rect::from_min_max(
                                Pos2::ZERO + from * pixels_per_point,
                                Pos2::ZERO + to * pixels_per_point,
                            ),
                        }),
                        ..template.clone()
                    }
                })
                .collect(),
        )
    })
    .flatten()
    .unwrap_or_default()
}

pub fn anchored_drawing(blits: AnchoredBlits) -> beui::native::Drawing {
    beui::native::painted_drawing(AnchoredDrawing::new(blits))
}

pub fn view_placed(app_id: &str, instance: EditorInstanceId, region: ViewId) -> Option<[u32; 3]> {
    with(app_id, |runtime| view_placed_in(runtime, instance, region)).flatten()
}

fn view_placed_in(
    runtime: &Runtime,
    instance: EditorInstanceId,
    region: ViewId,
) -> Option<[u32; 3]> {
    let screen = runtime.instances.screen_id(instance, region)?;
    runtime
        .shown_placement(screen)
        .map(|placement| [placement.surface, placement.width, placement.height])
}

pub fn view_damage(
    app_id: &str,
    instance: EditorInstanceId,
    region: ViewId,
    view: &PlacedView,
    pieces: &[Piece],
    rotation: f32,
    seen: u64,
) -> Option<Vec<Rect>> {
    if view.held.is_some() || rotation != 0.0 {
        return None;
    }
    with(app_id, |runtime| {
        let screen = runtime.instances.screen_id(instance, region)?;
        runtime
            .template(screen)
            .damage(pieces, view.rect.size(), seen)
    })
    .flatten()
}

pub fn view_published(app_id: &str) -> u64 {
    with(app_id, |runtime| runtime.shared.borrow().published.count()).unwrap_or_default()
}

pub(super) fn each(mut act: impl FnMut(&mut Runtime)) {
    HOST.with(|host| host.borrow_mut().runtimes.values_mut().for_each(&mut act));
}

pub(super) fn with<R>(app_id: &str, act: impl FnOnce(&mut Runtime) -> R) -> Option<R> {
    HOST.with(|host| {
        host.borrow_mut()
            .runtimes
            .get_mut(app_id)?
            .contain(|runtime| Some(act(runtime)))
    })
}

pub(crate) fn reported_color(color: &be_sdk_api::accessibility::Color) -> beui::color::Color32 {
    beui::color::Color32::from_rgba_unmultiplied(color.red, color.green, color.blue, color.alpha)
}

#[cfg(test)]
mod tests;
