use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use beui::reactive::EmbedPlacement;
use beui::{Pos2, Rect, Vec2, pos2, vec2};
use block_plugin_api::{
    ArtifactDescription, BlockPick, ChildPlacements, EditorInstanceId, EditorMessage, EditorRegion,
    FrameCommit, FrameSpec, HostAction, HostPanel, HostSession, HostValue, MAX_QUEUED_MESSAGES,
    Message, Monitor, PluginManifest, PresentedFrame, RegionSize, ScreenDamage, ScreenId,
    ScreenLayout, ScreenRequest, ScreenWindow, SessionState, SurfaceFormat, SurfaceSpec,
    ViewChange, decode_host, encode_host,
};
use uuid::Uuid;

use crate::host;

use super::{
    ArtifactSlot, ArtifactState, BlockPickRequest, CreationSlot, CreationState, HostChild,
    HostChildStatus, InstanceRole, RuntimeStatus, SurfaceStatus,
    backend::{Availability, Backend, Deadline, ShownFrame},
    input::{BlockDrag, BlockDragEvent},
    instances::{EditorView, Focus, Instances, Placement},
    presenter::{self, Blit, Piece, PresenterState, PresenterStatus, Quad, RegionDrawing, Shared},
    preview_scale,
};

const HOST_NAME: &str = "BE3";
const UNIT: Rect = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
const FRAME_TIMEOUT: Duration = Duration::from_secs(1);
const FRAME_BUDGET: Duration = Duration::from_millis(8);
const STOPPED_CLOCK_FRAME_BUDGET: Duration = Duration::from_secs(10);
pub(crate) const PACING: &str = "Frame pacing";
const REMEMBERED_PRESENTS: usize = 16;
const SURFACE: SurfaceSpec = SurfaceSpec {
    format: SurfaceFormat::Rgba8Unorm,
};

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::new());
}

struct Host {
    availability: Availability,
    deadline: Option<(u64, Deadline)>,
    waited: Duration,
    over_budget: u64,
    runtimes: HashMap<String, Runtime>,
    focus: Focus,
    published: HashMap<String, Arc<Vec<u8>>>,
    shell: Option<(String, EditorInstanceId)>,
    grabbed: bool,
}

impl Host {
    fn new() -> Self {
        Self {
            availability: Availability::missing(),
            deadline: None,
            waited: Duration::ZERO,
            over_budget: 0,
            runtimes: HashMap::new(),
            focus: Focus::default(),
            published: HashMap::new(),
            shell: None,
            grabbed: false,
        }
    }

    fn runtime(&mut self, plugin: &PluginManifest) -> Result<&mut Runtime, String> {
        if let Err(error) = &self.availability.0 {
            return Err(error.clone());
        }
        let surface = self.surface_for(&plugin.identity.id);
        let focus = self.focus.clone();
        let published = &self.published;
        let shell = self
            .shell
            .as_ref()
            .filter(|(shell, _)| *shell == plugin.identity.id)
            .map(|(_, instance)| *instance);
        let runtime = self
            .runtimes
            .entry(plugin.identity.id.clone())
            .or_insert_with(|| {
                let mut runtime = Runtime::new(plugin, surface);
                for (key, value) in published {
                    runtime.instances.set_host_value(key, Arc::clone(value));
                }
                runtime.instances.set_shell(shell);
                runtime
            });
        runtime.instances.set_focus(focus);
        let pass = host::pass();
        runtime.contain(|runtime| runtime.begin_pass(pass));
        Ok(runtime)
    }

    fn surface_for(&self, plugin_id: &str) -> u32 {
        if let Some(runtime) = self.runtimes.get(plugin_id) {
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

    fn shutdown(&mut self, plugin_id: &str) {
        let Some(mut runtime) = self.runtimes.remove(plugin_id) else {
            return;
        };
        runtime.stop();
        presenter::release(runtime.surface, &runtime.status);
        host::request_repaint();
    }
}

pub(super) struct Runtime {
    plugin: PluginManifest,
    pub(super) backend: Backend,
    session: HostSession,
    queued: Vec<Message>,
    held: Vec<Answer>,
    taking_in: bool,
    pub(super) instances: Instances,
    pub(super) layout: ScreenLayout,
    pub(super) pass: u64,
    since: u64,
    placed: u64,
    surface: u32,
    status: PresenterStatus,
    shared: Rc<RefCell<Shared>>,
    presented: bool,
    sent: Vec<ScreenRequest>,
    error: Option<String>,
    pacing: Pacing,
    preferences: block_plugin_api::Preferences,
    utc_offset: Option<i32>,
    fonts_sent: bool,
    described: bool,
    painting: bool,
    pub(super) placements: HashMap<(EditorInstanceId, EditorRegion), Placed>,
    offers: HashMap<(EditorInstanceId, EditorRegion), Vec<Offer>>,
    fallbacks: super::fonts::Fallbacks,
    presents: Presents,
    frames: u64,
}

enum Answer {
    Message(Message),
    Frame(FrameCommit),
    Laid(Laid),
}

struct Laid {
    serial: u64,
    sizes: Vec<RegionSize>,
    windows: Vec<ScreenWindow>,
    children: Vec<ChildPlacements>,
}

impl Laid {
    fn is_empty(&self) -> bool {
        self.sizes.is_empty() && self.windows.is_empty() && self.children.is_empty()
    }

    fn screens(&self, instances: &Instances) -> std::collections::HashSet<ScreenId> {
        self.sizes
            .iter()
            .map(|size| size.screen)
            .chain(self.windows.iter().map(|window| window.screen))
            .chain(self.children.iter().filter_map(|placements| {
                instances.screen_id(placements.instance, placements.region)
            }))
            .collect()
    }
}

pub(super) struct Placed {
    pub(super) rect: Rect,
    pub(super) clip: Rect,
    pub(super) zoom: f32,
    shown: Rect,
}

#[derive(Default)]
struct Pacing {
    needed: bool,
    unseen: bool,
    animating: bool,
    paint_at: Option<Duration>,
    requested_at: Option<Duration>,
    animated: u64,
}

impl Pacing {
    fn reset(&mut self) {
        self.needed = false;
        self.unseen = false;
        self.animating = false;
        self.paint_at = None;
        self.requested_at = None;
    }

    fn due(&self, now: Duration, pass: u64) -> bool {
        (self.needed || (self.animated != pass && self.paint_at.is_some_and(|at| at <= now)))
            && self
                .requested_at
                .is_none_or(|at| now.saturating_sub(at) >= FRAME_TIMEOUT)
    }

    fn request(&mut self, now: Duration, pass: u64) {
        self.requested_at = Some(now);
        self.animated = pass;
    }

    fn ready(&mut self, now: Duration, repaint_after: Option<Duration>) {
        self.needed = false;
        self.requested_at = None;
        self.paint_at = repaint_after.map(|delay| now + delay);
    }

    fn wake_after(&self, now: Duration, drawing: bool) -> Option<Duration> {
        match (self.requested_at, self.paint_at) {
            (Some(_), _) => Some(FRAME_TIMEOUT),
            (None, Some(at)) if drawing => Some(at.saturating_sub(now)),
            _ => None,
        }
    }
}

#[derive(Default)]
struct Presents {
    reported: VecDeque<PresentedFrame>,
    shown: u64,
}

impl Presents {
    fn report(&mut self, presented: PresentedFrame) {
        if self.reported.len() == REMEMBERED_PRESENTS {
            self.reported.pop_front();
        }
        self.reported.push_back(presented);
    }

    fn damage_through(&mut self, presents: u64) -> Option<Vec<ScreenDamage>> {
        let since = std::mem::replace(&mut self.shown, presents);
        let mut damage = Vec::new();
        let mut found = 0;
        for presented in &self.reported {
            if presented.sequence > since && presented.sequence <= presents {
                damage.extend_from_slice(&presented.damage);
                found += 1;
            }
        }
        self.reported
            .retain(|presented| presented.sequence > presents);
        (presents > since && found == presents - since).then_some(damage)
    }
}

impl Runtime {
    fn new(plugin: &PluginManifest, surface: u32) -> Self {
        let mut backend = Backend::new(plugin);
        backend.start(plugin);
        let mut instances = Instances::default();
        instances.allow_network(plugin.network.clone());
        instances.set_plugin_id(plugin.identity.id.clone());
        let session = session();
        Self {
            plugin: plugin.clone(),
            backend,
            session,
            queued: Vec::new(),
            held: Vec::new(),
            taking_in: false,
            instances,
            layout: ScreenLayout::default(),
            pass: 0,
            since: 0,
            placed: 0,
            surface,
            status: PresenterStatus::waiting(),
            shared: Rc::new(RefCell::new(Shared::default())),
            presented: false,
            sent: Vec::new(),
            error: None,
            pacing: Pacing::default(),
            preferences: host::preferences(),
            utc_offset: None,
            fonts_sent: false,
            described: false,
            painting: false,
            placements: HashMap::new(),
            offers: HashMap::new(),
            fallbacks: super::fonts::Fallbacks::new(),
            presents: Presents::default(),
            frames: 0,
        }
    }

    fn stop(&mut self) {
        self.session.shutdown(self.milliseconds());
        self.drain();
        self.backend.shutdown();
    }

    fn restart(&mut self) {
        mark(&self.plugin.identity.id);
        let plugin = self.plugin.clone();
        self.stop();
        self.session = session();
        self.queued.clear();
        self.held.clear();
        self.error = None;
        self.status = PresenterStatus::waiting();
        self.layout = ScreenLayout::default();
        self.sent.clear();
        self.pacing.reset();
        self.preferences = host::preferences();
        self.utc_offset = None;
        self.fonts_sent = false;
        self.described = false;
        self.painting = false;
        self.fallbacks = super::fonts::Fallbacks::new();
        self.presents = Presents::default();
        self.instances.reopen();
        self.backend.start(&plugin);
    }

    fn begin_pass(&mut self, pass: u64) {
        self.detect_error();
        if self.pass == pass || self.error.is_some() {
            return;
        }
        self.since = self.pass;
        self.pass = pass;
        let mut messages = Vec::new();
        if !self.fonts_sent && *self.session.state() == SessionState::Running {
            self.fonts_sent = true;
            messages.push(Message::Fonts(super::fonts::bundled()));
        }
        let describing = super::description::describing();
        if *self.session.state() == SessionState::Running && self.described != describing {
            self.described = describing;
            messages.push(Message::Describe(describing));
        }
        let preferences = host::preferences();
        if self.preferences != preferences {
            self.preferences = preferences;
            messages.push(Message::Preferences(preferences));
        }
        let utc_offset = crate::platform::utc_offset();
        if *self.session.state() == SessionState::Running && self.utc_offset != Some(utc_offset) {
            self.utc_offset = Some(utc_offset);
            messages.push(Message::UtcOffset(utc_offset));
        }
        self.update(messages);
    }

    fn update(&mut self, mut messages: Vec<Message>) {
        let drawing = self.session.granted_surface().is_some();
        let next = self.instances.next_screens(self.since);
        messages.extend(next.opened);
        if drawing && self.sent != next.screens {
            self.sent.clone_from(&next.screens);
            messages.push(self.instances.screen_set(next.screens));
        }
        messages.extend(self.instances.pending());
        messages.extend(self.instances.configure(&messages));
        let awaited = messages.iter().any(|message| {
            matches!(
                message,
                Message::Editor(EditorMessage::Open { .. } | EditorMessage::OpenArtifact { .. })
            )
        });
        self.pacing.needed |= !messages.is_empty();
        self.send(messages);
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
        self.pacing.needed |= !messages.is_empty();
        self.send(messages);
    }

    fn ask(&mut self, pass: u64) -> bool {
        if self.placed == pass || self.instances.has_mounted() {
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
        self.request_frame();
        true
    }

    fn start(&mut self, pass: u64) {
        self.detect_error();
        if self.error.is_some() || !(self.instances.has_mounted() || self.pass + 1 >= pass) {
            return;
        }
        self.pump();
        self.request_frame();
    }

    fn settle(&mut self, deadline: Deadline) -> bool {
        while self.error.is_none() && !self.backend.settled() && self.backend.wait(deadline) {
            self.pump();
            self.request_frame();
        }
        self.error.is_none() && !self.backend.settled()
    }

    pub(super) fn name(&self) -> &str {
        &self.plugin.identity.name
    }

    pub(super) fn id(&self) -> &str {
        &self.plugin.identity.id
    }

    pub(super) fn node_action(
        &mut self,
        screen: ScreenId,
        request: beui::accesskit::ActionRequest,
    ) {
        self.pacing.needed = true;
        self.send(vec![Message::NodeAction { screen, request }]);
        host::request_repaint();
    }

    pub(super) fn run_action(&mut self, screen: ScreenId, id: String) {
        self.pacing.needed = true;
        self.send(vec![Message::RunAction { screen, id }]);
        host::request_repaint();
    }

    fn busy(&mut self) -> bool {
        if self.error.is_some() || !self.instances.has_mounted() {
            return false;
        }
        let starting = matches!(
            self.session.state(),
            SessionState::Idle | SessionState::Starting
        );
        starting
            || self.painting
            || self.drawing()
            || !self.backend.settled()
            || self.instances.awaiting_content()
    }

    fn request_painting(&mut self) {
        if self.error.is_some()
            || !self.instances.has_mounted()
            || *self.session.state() != SessionState::Running
        {
            return;
        }
        self.painting = true;
        self.send(vec![Message::Extension {
            key: beui::painting::PAINTING_EXTENSION,
            payload: Vec::new(),
        }]);
    }

    fn painted(&mut self, payload: &[u8]) {
        self.painting = false;
        let mut shared = self.shared.borrow_mut();
        shared.paintings.clear();
        shared.overflows.clear();
        let Ok(paintings) = beui::painting::ScreenPaintings::decode(payload) else {
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
        let drawing = self.session.granted_surface().is_some();
        drawing && (self.pacing.needed || self.pacing.requested_at.is_some())
    }

    fn request_frame(&mut self) {
        if self.error.is_some() {
            return;
        }
        let drawing = self.session.granted_surface().is_some();
        let seen = host::seen();
        self.pacing.needed |= seen && self.pacing.unseen;
        if drawing && self.pacing.due(self.now(), host::pass()) {
            self.pacing.request(self.now(), host::pass());
            self.pacing.unseen = !seen;
            self.send(vec![Message::DrawFrame {
                now_micros: self.now().as_micros() as u64,
                seen,
            }]);
        }
        if let Some(delay) = self.pacing.wake_after(self.now(), drawing) {
            host::request_repaint_after(delay);
        }
        if drawing && self.pacing.animating {
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
        match crate::panic_guard::contain(|| act(self)) {
            Ok(done) => done,
            Err(panic) => {
                self.crashed(format!("The app failed while running it: {panic}"));
                R::default()
            }
        }
    }

    fn crashed(&mut self, error: String) {
        if self.error.is_some() {
            return;
        }
        let first = error.lines().next().unwrap_or_default();
        crate::notices::report_with(format!("{} stopped: {first}", self.name()), error.clone());
        self.error = Some(error);
        let _ = crate::panic_guard::contain(|| self.backend.shutdown());
        CRASHED.with(|crashed| crashed.borrow_mut().push(self.plugin.identity.id.clone()));
        mark(&self.plugin.identity.id);
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
        self.error = match self.status.get() {
            PresenterState::Failed(error) | PresenterState::Unsupported(error) => Some(error),
            PresenterState::Waiting | PresenterState::Presenting | PresenterState::Released => None,
        };
        if self.error.is_some() {
            mark(&self.plugin.identity.id);
        }
    }

    fn pump(&mut self) {
        if self.error.is_some() {
            return;
        }
        let now = self.milliseconds();
        let before = self.session.state().clone();
        let received = self.backend.receive();
        if *self.session.state() == SessionState::Idle && self.backend.ready() {
            self.session.start(now);
        }
        let mut forwarded = Vec::with_capacity(received.len());
        for message in received {
            match message.is_session() {
                true => self.session.receive(message, now),
                false => forwarded.push(message),
            }
        }
        self.deliver();
        self.session.tick(now);
        if *self.session.state() != before {
            mark(&self.plugin.identity.id);
            host::request_repaint();
        }
        if let Some(deadline) = self.session.next_deadline() {
            host::request_repaint_after(Duration::from_millis(deadline.saturating_sub(now)));
        }
        match self.session.state() {
            SessionState::Idle | SessionState::Starting | SessionState::Running => {}
            SessionState::Failed(failure) => {
                let error = format!("The plugin session stopped: {failure:?}");
                self.crashed(error);
                return;
            }
            state => {
                self.error = Some(format!("The plugin session stopped: {state:?}"));
                return;
            }
        }
        let forwarded = match self.taking_in {
            true => {
                let mut taken = std::mem::take(&mut self.held);
                taken.extend(forwarded.into_iter().map(Answer::Message));
                taken
            }
            false => {
                let mut drawing = Vec::with_capacity(forwarded.len());
                let mut held = false;
                for message in forwarded {
                    match message {
                        Message::Frame(commit) => {
                            let (now, later) = self.split(commit);
                            drawing.push(Answer::Frame(now));
                            if let Some(later) = later {
                                held = true;
                                self.held.push(Answer::Laid(later));
                            }
                        }
                        message if self.draws_the_frame(&message) => {
                            drawing.push(Answer::Message(message));
                        }
                        message => {
                            held = true;
                            self.held.push(Answer::Message(message));
                        }
                    }
                }
                if held {
                    host::request_repaint();
                }
                drawing
            }
        };
        self.apply(forwarded);
        if let Some(took) = self.backend.took() {
            let id = &self.plugin.identity.id;
            for (name, duration) in [
                ("step", took.step),
                ("GPU calls", took.gpu),
                ("submit", took.submit),
            ] {
                crate::performance::record_group_duration(
                    PACING,
                    &format!("{id} {name}"),
                    duration,
                );
            }
        }
        if let Some(mut frame) = self.backend.received_frame() {
            frame.set_damage(self.presents.damage_through(frame.presents()));
            self.shared.borrow_mut().publish(&self.layout, Some(frame));
            self.frames += 1;
            crate::performance::record_group_count(
                PACING,
                &format!("{} frames", self.plugin.identity.id),
                self.frames,
            );
            mark(&self.plugin.identity.id);
        }
    }

    fn draws_the_frame(&self, message: &Message) -> bool {
        match message {
            Message::Layout(_) | Message::FrameNeeded => true,
            Message::Extension { key, .. } => *key == beui::painting::PAINTING_EXTENSION,
            _ => false,
        }
    }

    fn split(&self, commit: FrameCommit) -> (FrameCommit, Option<Laid>) {
        let mut message = Message::Frame(commit);
        self.instances.translate(&mut message, true);
        let Message::Frame(mut commit) = message else {
            unreachable!("translating a frame leaves it a frame")
        };
        let (moving, coming) = std::mem::take(&mut commit.children)
            .into_iter()
            .partition(|placements| {
                !self.held.iter().any(|held| {
                    matches!(held, Answer::Laid(laid) if laid.children.iter().any(|earlier| {
                        (earlier.instance, earlier.region) == (placements.instance, placements.region)
                    }))
                }) && self.instances.only_moves(placements)
            });
        commit.children = moving;
        let later = Laid {
            serial: commit.serial,
            sizes: std::mem::take(&mut commit.sizes),
            windows: std::mem::take(&mut commit.windows),
            children: coming,
        };
        (commit, (!later.is_empty()).then_some(later))
    }

    fn held_screens(&self) -> std::collections::HashSet<ScreenId> {
        self.held
            .iter()
            .filter_map(|held| match held {
                Answer::Laid(laid) => Some(laid.screens(&self.instances)),
                Answer::Message(_) | Answer::Frame(_) => None,
            })
            .flatten()
            .collect()
    }

    fn lay(&mut self, laid: Laid) -> bool {
        let screens = laid.screens(&self.instances);
        let mut changed = self.instances.set_region_sizes(laid.sizes);
        changed |= self.instances.set_windows(laid.windows);
        for placements in laid.children {
            changed |= self.instances.set_children(placements);
        }
        self.instances.laid(laid.serial, &screens);
        changed
    }

    fn apply(&mut self, answers: Vec<Answer>) {
        if answers.is_empty() {
            return;
        }
        let mut replies = Vec::new();
        let mut changed = false;
        mark(&self.plugin.identity.id);
        for answer in answers {
            let mut message = match answer {
                Answer::Message(message) => message,
                Answer::Frame(commit) => {
                    changed |= self.commit(commit);
                    continue;
                }
                Answer::Laid(laid) => {
                    changed |= self.lay(laid);
                    continue;
                }
            };
            self.instances.translate(&mut message, true);
            changed |= match message {
                Message::Layout(layout) => {
                    self.layout = layout;
                    true
                }
                Message::Editor(message) => self.instances.editor_message(message),
                Message::FrameNeeded => {
                    self.pacing.needed = true;
                    true
                }
                Message::Frame(commit) => self.commit(commit),
                Message::MissingCharacters(missing) => {
                    replies.extend(self.fallbacks.answer(&missing).map(Message::Fonts));
                    false
                }
                Message::Extension { key, payload }
                    if key == beui::painting::PAINTING_EXTENSION =>
                {
                    self.painted(&payload);
                    true
                }
                _ => false,
            };
        }
        self.send(replies);
        if changed {
            host::request_repaint();
        }
    }

    fn commit(&mut self, commit: FrameCommit) -> bool {
        let FrameCommit {
            serial,
            sizes,
            reports,
            windows,
            children,
            repaint_after_micros,
            animating,
            presented,
        } = commit;
        self.pacing.animating = animating;
        self.await_next_frame(repaint_after_micros);
        if let Some(presented) = presented {
            self.presents.report(presented);
        }
        let held = self.held_screens();
        let mut changed = self.instances.confirm(serial, &held);
        changed |= self.instances.set_frame_reports(reports);
        changed |= self.lay(Laid {
            serial,
            sizes,
            windows,
            children,
        });
        changed
    }

    fn await_next_frame(&mut self, repaint_after_micros: Option<u64>) {
        let delay = repaint_after_micros.map(Duration::from_micros);
        self.pacing.ready(self.now(), delay);
        if let Some(delay) = delay {
            host::request_repaint_after(delay);
        }
    }

    fn send(&mut self, messages: Vec<Message>) {
        if messages.is_empty() || self.error.is_some() {
            return;
        }
        let mut messages = self.instances.hold_unreported(messages);
        for message in &mut messages {
            self.instances.translate(message, false);
        }
        self.queued.extend(self.instances.gate(messages));
        self.deliver();
    }

    fn release_reported(&mut self) {
        let messages = self.instances.take_reported();
        self.send(messages);
    }

    fn deliver(&mut self) {
        if self.session.state() != &SessionState::Running {
            self.drain();
            return;
        }
        let now = self.milliseconds();
        let mut failure = None;
        for message in std::mem::take(&mut self.queued) {
            if self.session.queued_message_count() == MAX_QUEUED_MESSAGES {
                self.drain();
            }
            if let Err(error) = self.session.send(message, now) {
                failure = Some(format!("The plugin message queue failed: {error:?}"));
                break;
            }
        }
        self.drain();
        if let Some(failure) = failure {
            self.error.get_or_insert(failure);
        }
    }

    fn drain(&mut self) {
        let mut outbound = Vec::new();
        while let Some(message) = self.session.next_outbound() {
            outbound.push(message);
        }
        if !outbound.is_empty() {
            self.backend.send(outbound);
        }
    }

    fn flush(&mut self) {
        self.pump();
        let presented = std::mem::take(&mut self.presented) || self.instances.has_mounted();
        if !presented {
            return;
        }
        self.shared.borrow_mut().publish(&self.layout, None);
    }

    fn template(&self, screen: ScreenId, drawn: Option<(u32, u32)>) -> Blit {
        Blit {
            surface: self.surface,
            status: self.status.clone(),
            shared: Rc::clone(&self.shared),
            screen,
            quad: Quad::upright(Rect::ZERO),
            source: UNIT,
            drawn,
            requested: self
                .sent
                .iter()
                .find(|request| request.screen == screen)
                .map(|request| (request.metrics.pixel_width, request.metrics.pixel_height)),
            placed: self
                .layout
                .placement(screen)
                .map(|placement| [placement.surface, placement.width, placement.height]),
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
pub(crate) fn place_screens(blits: &[Blit], scale: f32, background: beui::Color32) {
    super::web::set_background(background);
    let mut placed = HashMap::<u32, Vec<super::web::Placement>>::new();
    for (order, blit) in blits.iter().enumerate() {
        if let Some(placement) = super::web::placement(blit, order, scale) {
            placed.entry(blit.surface).or_default().push(placement);
        }
    }
    HOST.with(|host| {
        for runtime in host.borrow_mut().runtimes.values_mut() {
            let placements = placed.remove(&runtime.surface).unwrap_or_default();
            runtime.backend.place(placements);
        }
    });
}

pub(crate) fn install(setup: &beui::Setup) {
    let availability = presenter::install(setup);
    HOST.with(|host| {
        host.borrow_mut().availability = availability;
    });
}

pub(crate) fn exiting() {
    let runtimes = HOST.with(|host| {
        let mut host = host.borrow_mut();
        host.availability = Availability::missing();
        std::mem::take(&mut host.runtimes)
    });
    for (_, mut runtime) in runtimes {
        runtime.stop();
    }
    presenter::stop();
}

pub(crate) fn replace_gpu(setup: &beui::Setup) {
    install(setup);
    HOST.with(|host| {
        for runtime in host.borrow_mut().runtimes.values_mut() {
            runtime.restart();
        }
    });
}

pub(crate) struct HostFrame {
    pub(crate) content: Rect,
    pub(crate) floating: Vec<Rect>,
}

pub(crate) fn report_child_bars(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    actions: Vec<(block_plugin_api::ChildId, block_plugin_api::BarAction)>,
) {
    if actions.is_empty() {
        return;
    }
    with(plugin_id, |runtime| {
        let messages = runtime
            .instances
            .child_bar_actions(instance, region, actions);
        runtime.send(messages);
    });
}

pub(crate) fn report_child_focus(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    child: block_plugin_api::ChildId,
) {
    with(plugin_id, |runtime| {
        runtime.send(vec![Message::Editor(EditorMessage::ChildFocused {
            instance,
            region,
            child,
        })]);
    });
}

pub(crate) fn report_children(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    statuses: Vec<HostChildStatus>,
) {
    with(plugin_id, |runtime| {
        let messages = runtime
            .instances
            .set_child_statuses(instance, region, statuses);
        if !messages.is_empty() {
            runtime.pacing.needed = true;
            host::request_repaint();
        }
        runtime.send(messages);
    });
}

pub(crate) fn take_block_pick(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Option<BlockPickRequest> {
    with(plugin_id, |runtime| {
        runtime.instances.take_block_pick(instance)
    })
    .flatten()
}

pub(crate) fn take_pick_answers(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Vec<(u64, BlockPick)> {
    with(plugin_id, |runtime| {
        runtime.instances.take_pick_answers(instance)
    })
    .unwrap_or_default()
}

pub(crate) fn take_child_commits(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Vec<super::ChildCommit> {
    with(plugin_id, |runtime| {
        runtime.instances.take_child_commits(instance)
    })
    .unwrap_or_default()
}

pub(crate) struct Pick {
    pub(crate) pick: u64,
    pub(crate) filter: block_plugin_api::BlockFilter,
    pub(crate) parent: block_plugin_api::BlockLocation,
    pub(crate) within: Option<Uuid>,
    pub(crate) touch: bool,
}

pub(crate) fn request_pick(plugin_id: &str, instance: EditorInstanceId, pick: Pick) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.request_pick(instance, pick);
        runtime.send(messages);
    });
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FramePlacement {
    pub(crate) rect: Rect,
    pub(crate) zoom: f32,
}

pub(crate) fn frame_placement(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Option<FramePlacement> {
    with(plugin_id, |runtime| {
        runtime
            .placements
            .get(&(instance, EditorRegion::Frame))
            .map(|placed| FramePlacement {
                rect: placed.rect,
                zoom: placed.zoom,
            })
    })
    .flatten()
}

pub(crate) fn reanchor(anchor: Rect, from: FramePlacement, to: FramePlacement) -> Option<Rect> {
    let scale = from.zoom / to.zoom;
    let moved = Rect::from_min_size(
        Pos2::ZERO + (from.rect.min + anchor.min.to_vec2() * from.zoom - to.rect.min) / to.zoom,
        anchor.size().max(Vec2::ZERO) * scale,
    );
    [moved.min.x, moved.min.y, moved.max.x, moved.max.y]
        .iter()
        .all(|value| value.is_finite())
        .then_some(moved)
}

pub(crate) fn block_picked(
    plugin_id: &str,
    instance: EditorInstanceId,
    request_id: u64,
    pick: BlockPick,
) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.block_picked(instance, request_id, pick);
        runtime.send(messages);
    });
}

fn scale_rect(rect: Rect, size: Vec2) -> Rect {
    Rect::from_min_max(
        pos2(rect.min.x * size.x, rect.min.y * size.y),
        pos2(rect.max.x * size.x, rect.max.y * size.y),
    )
}

pub(crate) fn creation(slot: CreationSlot<'_>) -> CreationState {
    let CreationSlot {
        plugin,
        block_types,
        client_id,
        instance,
        role,
    } = slot;
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let runtime = match host.runtime(plugin) {
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

pub(crate) fn artifact(slot: ArtifactSlot<'_>) -> ArtifactState {
    let ArtifactSlot {
        plugin,
        block_types,
        client_id,
        instance,
        editor,
        block,
        data,
        resync,
    } = slot;
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let runtime = match host.runtime(plugin) {
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
                data,
                resync,
            );
            runtime.send(messages);
            runtime.release_reported();
            runtime.instances.artifact_description(instance)
        });
        match (description, runtime.error.clone()) {
            (_, Some(error)) => ArtifactState::Failed(error),
            (Some(ArtifactDescription::Described { source, summary }), None) => {
                ArtifactState::Described {
                    source: Uuid::from_bytes(source),
                    summary,
                }
            }
            (Some(ArtifactDescription::Unreadable(error)), None) => ArtifactState::Failed(error),
            (None, None) => ArtifactState::Starting,
        }
    })
}

pub(crate) fn poll() {
    let pass = host::pass();
    for command in host::take_commands() {
        match command {
            host::HostCommand::RestartPlugin(plugin_id) => {
                with(&plugin_id, Runtime::restart);
            }
        }
    }
    let touched = crate::be::take_touched();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        for runtime in host.runtimes.values_mut() {
            runtime.contain(|runtime| {
                runtime.instances.touch(&touched);
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
            crate::host::set_grab(grabbed);
        }
    });
}

pub(crate) fn settle() {
    let pass = host::pass();
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let deadline = match host.deadline {
            Some((at, deadline)) if at == pass => deadline,
            _ => Deadline::after(match host::clock_stopped() {
                true => STOPPED_CLOCK_FRAME_BUDGET,
                false => FRAME_BUDGET,
            }),
        };
        host.deadline = Some((pass, deadline));
        let asked: Vec<String> = host
            .runtimes
            .iter_mut()
            .filter_map(|(id, runtime)| {
                runtime
                    .contain(|runtime| runtime.ask(pass))
                    .then(|| id.clone())
            })
            .collect();
        let started = std::time::Instant::now();
        let mut over = false;
        for id in asked {
            if let Some(runtime) = host.runtimes.get_mut(&id) {
                over |= runtime.contain(|runtime| runtime.settle(deadline));
            }
        }
        host.waited += started.elapsed();
        host.over_budget += u64::from(over);
    });
}

pub(crate) fn busy() -> bool {
    HOST.with(|host| {
        host.borrow_mut()
            .runtimes
            .values_mut()
            .any(|runtime| runtime.contain(Runtime::busy))
    })
}

pub(crate) fn request_painting() {
    each(|runtime| {
        runtime.contain(|runtime| {
            runtime.request_painting();
            Some(())
        });
    });
    host::request_repaint();
}

pub(crate) fn drawing() -> bool {
    HOST.with(|host| {
        host.borrow_mut()
            .runtimes
            .values_mut()
            .any(|runtime| runtime.contain(Runtime::drawing))
    })
}

pub(crate) fn start_frames() {
    let pass = host::pass();
    HOST.with(|host| {
        for runtime in host.borrow_mut().runtimes.values_mut() {
            runtime.contain(|runtime| runtime.start(pass));
        }
    });
}

pub(crate) fn record_pacing() {
    let (waited, over_budget) = HOST.with(|host| {
        let mut host = host.borrow_mut();
        (std::mem::take(&mut host.waited), host.over_budget)
    });
    crate::performance::record_group_duration(PACING, "Waiting on plugins", waited);
    crate::performance::record_group_count(PACING, "Frames over budget", over_budget);
}

pub(crate) fn flush() {
    HOST.with(|host| {
        for runtime in host.borrow_mut().runtimes.values_mut() {
            runtime.contain(Runtime::flush);
        }
    });
}

pub(crate) fn set_focus(block: Option<(Uuid, Uuid)>, via: Vec<Uuid>) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let focus = Focus { block, via };
        if host.focus == focus {
            return;
        }
        host.focus = focus.clone();
        for runtime in host.runtimes.values_mut() {
            if runtime.instances.set_focus(focus.clone()) {
                runtime.pacing.needed = true;
            }
        }
    });
}

pub(crate) fn publish<T: HostValue>(value: &T::Value) {
    let bytes = encode_host(value);
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        if host
            .published
            .get(T::KEY)
            .is_some_and(|published| **published == bytes)
        {
            return;
        }
        let value = Arc::new(bytes);
        host.published.insert(T::KEY.to_owned(), Arc::clone(&value));
        for (plugin_id, runtime) in &mut host.runtimes {
            if runtime.instances.set_host_value(T::KEY, Arc::clone(&value)) {
                runtime.pacing.needed = true;
                mark(plugin_id);
                host::request_repaint();
            }
        }
    });
}

#[cfg(target_os = "linux")]
pub(crate) fn watched<T: HostValue>() -> bool {
    HOST.with(|host| {
        host.borrow()
            .runtimes
            .values()
            .any(|runtime| runtime.instances.watches_host_value(T::KEY))
    })
}

pub(crate) fn take_actions<A: HostAction>() -> Vec<A> {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let mut runtimes: Vec<_> = host.runtimes.iter_mut().collect();
        runtimes.sort_by_key(|(left, _)| *left);
        runtimes
            .into_iter()
            .flat_map(|(_, runtime)| runtime.instances.take_host_actions(A::KEY))
            .filter_map(|action| decode_host(&action))
            .collect()
    })
}

pub(crate) fn set_shell(shell: Option<(&str, EditorInstanceId)>) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let shell = shell.map(|(plugin_id, instance)| (plugin_id.to_owned(), instance));
        if host.shell == shell {
            return;
        }
        host.shell = shell.clone();
        for (plugin_id, runtime) in &mut host.runtimes {
            let instance = shell
                .as_ref()
                .filter(|(shell, _)| shell == plugin_id)
                .map(|(_, instance)| *instance);
            if runtime.instances.set_shell(instance) {
                runtime.pacing.needed = true;
                mark(plugin_id);
                host::request_repaint();
            }
        }
    });
}

pub(crate) fn take_focus_report(plugin_id: &str, instance: EditorInstanceId) -> Option<Focus> {
    with(plugin_id, |runtime| {
        runtime.instances.take_focus_report(instance)
    })
    .flatten()
}

pub(crate) fn take_artifact_watch(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Option<Vec<Uuid>> {
    with(plugin_id, |runtime| {
        runtime.instances.take_artifact_watch(instance)
    })
    .flatten()
}

pub(crate) fn menu(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Vec<block_plugin_api::MenuEntry> {
    with(plugin_id, |runtime| runtime.instances.menu(instance)).unwrap_or_default()
}

pub(crate) fn menu_pick(plugin_id: &str, instance: EditorInstanceId, id: String) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.menu_pick(instance, id);
        runtime.send(messages);
    });
}

pub(crate) fn take_child_menu_picks(
    plugin_id: &str,
    instance: EditorInstanceId,
    children: &[block_plugin_api::ChildId],
) -> Vec<(block_plugin_api::ChildId, String)> {
    with(plugin_id, |runtime| {
        runtime.instances.take_child_menu_picks(instance, children)
    })
    .unwrap_or_default()
}

pub(crate) fn show_dialog(
    plugin_id: &str,
    instance: EditorInstanceId,
    block: Uuid,
    dialog: block_plugin_api::ShellDialog,
) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.show_dialog(instance, block, dialog);
        runtime.send(messages);
    });
}

pub(crate) fn show_panel(plugin_id: &str, instance: EditorInstanceId, panel: HostPanel) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.show_panel(instance, panel);
        runtime.send(messages);
    });
}

pub(crate) fn show_block(
    plugin: &PluginManifest,
    instance: EditorInstanceId,
    open: crate::editors::BlockOpen,
) {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let Ok(runtime) = host.runtime(plugin) else {
            return;
        };
        runtime.contain(|runtime| {
            let messages = runtime.instances.show_block(instance, open);
            runtime.send(messages);
        });
    });
    mark(&plugin.identity.id);
}

pub(crate) fn set_artifact_states(
    plugin_id: &str,
    instance: EditorInstanceId,
    states: Vec<block_plugin_api::ArtifactState>,
) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.set_artifact_states(instance, states);
        runtime.send(messages);
    });
}

pub(crate) fn kill(plugin_id: &str) {
    HOST.with(|host| {
        host.borrow_mut().shutdown(plugin_id);
    });
    mark(plugin_id);
}

pub(crate) fn close(plugin_id: &str, instance: EditorInstanceId) {
    super::instances::want_presence(instance, false);
    with(plugin_id, |runtime| {
        let messages = runtime
            .instances
            .remove(instance)
            .then(|| vec![Message::Editor(EditorMessage::Close { instance })]);
        if let Some(messages) = messages {
            runtime.send(messages);
        }
    });
    mark(plugin_id);
}

pub(crate) fn artifact_draft(plugin_id: &str, instance: EditorInstanceId) -> Option<Vec<u8>> {
    with(plugin_id, |runtime| {
        runtime.instances.artifact_draft(instance)
    })
    .flatten()
}

pub(crate) fn regenerate_artifact(plugin_id: &str, instance: EditorInstanceId, data: &[u8]) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.regenerate_artifact(instance, data);
        runtime.send(messages);
    });
}

pub(crate) fn take_artifact_outcome(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Option<Result<(), String>> {
    with(plugin_id, |runtime| {
        runtime.instances.take_artifact_outcome(instance)
    })
    .flatten()
}

pub(crate) fn region_size(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
) -> Option<Vec2> {
    with(plugin_id, |runtime| {
        runtime.instances.region_size(instance, region)
    })
    .flatten()
}

pub(crate) fn creation_ready(plugin_id: &str, instance: EditorInstanceId) -> bool {
    with(plugin_id, |runtime| {
        runtime.instances.creation_ready(instance)
    })
    .unwrap_or_default()
}

pub(crate) fn commit_creation(plugin_id: &str, instance: EditorInstanceId) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.commit_creation(instance);
        runtime.send(messages);
    });
}

pub(crate) fn take_created(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Option<Result<Uuid, String>> {
    with(plugin_id, |runtime| {
        runtime.instances.take_created(instance)
    })
    .flatten()
}

pub(crate) fn frame_child(plugin_id: &str, instance: EditorInstanceId) -> Option<Uuid> {
    with(plugin_id, |runtime| runtime.instances.frame_child(instance)).flatten()
}

pub(crate) fn revoke_frame_child(plugin_id: &str, instance: EditorInstanceId) {
    with(plugin_id, |runtime| {
        runtime.instances.revoke_frame_child(instance);
    });
    mark(plugin_id);
}

pub(crate) fn take_leaving(plugin_id: &str, instance: EditorInstanceId) -> bool {
    with(plugin_id, |runtime| {
        runtime.instances.take_leaving(instance)
    })
    .unwrap_or_default()
}

pub(crate) fn hold(plugin_id: &str, instance: EditorInstanceId, region: EditorRegion) {
    with(plugin_id, |runtime| {
        runtime.instances.hold(instance, region)
    });
}

pub(crate) fn frame_rects(plugin_id: &str, instance: EditorInstanceId) -> Option<HostFrame> {
    with(plugin_id, |runtime| {
        runtime
            .instances
            .frame_report(instance, EditorRegion::Frame)
            .map(|report| HostFrame {
                content: report.content,
                floating: report.floating.clone(),
            })
    })
    .flatten()
}

pub(crate) fn presenting(plugin_id: &str, instance: EditorInstanceId) -> bool {
    with(plugin_id, |runtime| runtime.instances.presenting(instance)).unwrap_or_default()
}

pub(crate) fn present(plugin_id: &str, instance: EditorInstanceId, presenting: bool) {
    with(plugin_id, |runtime| {
        if runtime.instances.set_presenting(instance, presenting) {
            mark(plugin_id);
            host::request_repaint();
        }
    });
}

pub(crate) fn set_presence_visible(plugin_id: &str, instance: EditorInstanceId, visible: bool) {
    super::instances::want_presence(instance, visible);
    with(plugin_id, |runtime| {
        let messages = runtime.instances.set_presence_visible(instance, visible);
        runtime.send(messages);
    });
}

pub(crate) fn replace_child(
    plugin_id: &str,
    instance: EditorInstanceId,
    old: Uuid,
    new: Uuid,
) -> Option<bool> {
    with(plugin_id, |runtime| {
        let (messages, replaced) = runtime.instances.replace_child(instance, old, new);
        runtime.send(messages);
        replaced
    })
    .flatten()
}

pub(crate) fn resized(plugin_id: &str, instance: EditorInstanceId, size: Vec2) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.resized(instance, size);
        runtime.send(messages);
    });
}

pub(crate) fn take_view_changes(plugin_id: &str, instance: EditorInstanceId) -> Vec<ViewChange> {
    with(plugin_id, |runtime| {
        runtime.instances.take_view_changes(instance)
    })
    .unwrap_or_default()
}

pub(crate) fn take_bar_actions(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Vec<block_plugin_api::BarAction> {
    with(plugin_id, |runtime| {
        runtime.instances.take_bar_actions(instance)
    })
    .unwrap_or_default()
}

pub(crate) fn aspect_ratio(plugin_id: &str, instance: EditorInstanceId) -> Option<f32> {
    with(plugin_id, |runtime| {
        runtime.instances.aspect_ratio(instance)
    })
    .flatten()
}

pub(crate) fn assigned_size(plugin_id: &str, instance: EditorInstanceId) -> Option<Vec2> {
    with(plugin_id, |runtime| {
        runtime.instances.assigned_size(instance)
    })
    .flatten()
}

pub(crate) fn intrinsic_size(plugin_id: &str, instance: EditorInstanceId) -> Option<Vec2> {
    with(plugin_id, |runtime| {
        runtime.instances.intrinsic_size(instance)
    })
    .flatten()
}

pub(crate) fn warm(plugin: &PluginManifest) {
    HOST.with(|host| {
        let _ = host.borrow_mut().runtime(plugin);
    });
}

pub(crate) fn in_use() -> Vec<String> {
    HOST.with(|host| {
        let mut in_use: Vec<String> = host
            .borrow()
            .runtimes
            .iter()
            .filter(|(_, runtime)| !runtime.instances.is_empty())
            .map(|(plugin_id, _)| plugin_id.clone())
            .collect();
        in_use.sort_unstable();
        in_use
    })
}

pub(crate) fn running() -> Vec<RuntimeStatus> {
    HOST.with(|host| {
        let host = host.borrow();
        let mut running: Vec<_> = host
            .runtimes
            .iter()
            .map(|(plugin_id, runtime)| RuntimeStatus {
                plugin_id: plugin_id.clone(),
                state: runtime.state(),
                surface: SurfaceStatus {
                    index: runtime.surface,
                    serial: runtime.instances.serial(),
                    pixels: runtime
                        .layout
                        .screens
                        .iter()
                        .map(|placement| u64::from(placement.width) * u64::from(placement.height))
                        .sum(),
                    placements: runtime.layout.screens.len(),
                },
                pass: runtime.pass,
                uptime: runtime.backend.uptime(),
                instances: runtime.instances.statuses(&runtime.layout, runtime.pass),
            })
            .collect();
        running.sort_by(|left, right| left.plugin_id.cmp(&right.plugin_id));
        running
    })
}

fn session() -> HostSession {
    HostSession::new(HOST_NAME, Some(SURFACE), host::preferences())
}

thread_local! {
    static CHANGED: RefCell<std::collections::HashSet<String>> =
        RefCell::new(std::collections::HashSet::new());
    static CRASHED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn take_crashed() -> Vec<String> {
    CRASHED.with(|crashed| std::mem::take(&mut *crashed.borrow_mut()))
}

pub(crate) fn crash(plugin_id: &str, error: String) {
    with(plugin_id, |runtime| runtime.crashed(error));
}

pub(super) fn mark(plugin_id: &str) {
    CHANGED.with(|changed| {
        if !changed.borrow().contains(plugin_id) {
            changed.borrow_mut().insert(plugin_id.to_owned());
        }
    });
}

pub(crate) fn take_changed() -> Vec<String> {
    CHANGED.with(|changed| changed.borrow_mut().drain().collect())
}

pub(crate) struct RegionSlot<'a> {
    pub(crate) plugin: &'a PluginManifest,
    pub(crate) block_types: &'a Arc<block_plugin_api::Catalog>,
    pub(crate) client_id: Uuid,
    pub(crate) role: InstanceRole,
    pub(crate) instance: EditorInstanceId,
    pub(crate) region: EditorRegion,
}

pub(crate) fn mount_region(slot: RegionSlot<'_>) {
    let RegionSlot {
        plugin,
        block_types,
        client_id,
        role,
        instance,
        region,
    } = slot;
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let Ok(runtime) = host.runtime(plugin) else {
            mark(&plugin.identity.id);
            return;
        };
        runtime.contain(|runtime| {
            runtime
                .instances
                .mount(instance, region, client_id, role, block_types);
            runtime.release_reported();
        });
        mark(&plugin.identity.id);
    });
    host::request_repaint();
}

pub(crate) fn unmount_region(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    mount: u64,
) {
    with(plugin_id, |runtime| {
        runtime.instances.unmount(instance, region);
        withdraw(runtime, instance, region, mount);
    });
    mark(plugin_id);
    host::request_repaint();
}

pub(crate) fn unplace_region(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    mount: u64,
) {
    with(plugin_id, |runtime| {
        withdraw(runtime, instance, region, mount)
    });
    mark(plugin_id);
    host::request_repaint();
}

fn withdraw(runtime: &mut Runtime, instance: EditorInstanceId, region: EditorRegion, mount: u64) {
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
pub(crate) struct RegionPlacement {
    pub(crate) rect: Rect,
    pub(crate) clip: Rect,
    pub(crate) zoom: f32,
    pub(crate) screens: Vec<beui::Screen>,
}

pub(super) struct Offer {
    mount: u64,
    placement: RegionPlacement,
    frame: Option<FrameSpec>,
    view: Option<EditorView>,
}

pub(crate) fn place_region(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    mount: u64,
    placement: RegionPlacement,
    frame: Option<FrameSpec>,
    view: Option<EditorView>,
) {
    with(plugin_id, |runtime| {
        if !runtime.instances.mounted(instance, region) {
            return;
        }
        let offers = runtime.offers.entry((instance, region)).or_default();
        let offer = Offer {
            mount,
            placement,
            frame,
            view,
        };
        match offers.iter_mut().find(|held| held.mount == mount) {
            Some(held) => *held = offer,
            None => offers.push(offer),
        }
        apply_offers(runtime, instance, region);
        mark(plugin_id);
    });
    host::request_repaint();
}

fn apply_offers(runtime: &mut Runtime, instance: EditorInstanceId, region: EditorRegion) {
    let Some(offers) = runtime.offers.get(&(instance, region)) else {
        return;
    };
    let area = |offer: &Offer| offer.placement.rect.width() * offer.placement.rect.height();
    let Some(chosen) = offers.iter().max_by(|a, b| area(a).total_cmp(&area(b))) else {
        return;
    };
    let shown = offers
        .iter()
        .filter_map(|offer| {
            Quad::upright(offer.placement.rect)
                .crop_to(offer.placement.clip)
                .map(|(_, source)| source)
        })
        .reduce(|shown, source| shown.union(source));
    let frame = chosen.frame;
    let view = chosen.view;
    let RegionPlacement {
        rect,
        clip,
        zoom,
        ref screens,
    } = chosen.placement;
    let screens = screens.clone();
    runtime.placed = host::pass();
    let zoom = match zoom.is_finite() && zoom > 0.0 && region != EditorRegion::Preview {
        true => zoom,
        false => 1.0,
    };
    runtime.placements.insert(
        (instance, region),
        Placed {
            rect,
            clip,
            zoom,
            shown: shown.unwrap_or(Rect::ZERO),
        },
    );
    let (size, scale_factor) = match region {
        EditorRegion::Preview => (
            rect.size(),
            preview_scale(rect.size(), host::pixels_per_point()),
        ),
        _ => (rect.size() / zoom, host::pixels_per_point() * zoom),
    };
    let visible = shown.map_or(Rect::ZERO, |source| scale_rect(source, size));
    let monitors = region_monitors(&screens, rect, size);
    runtime.instances.place_mounted(
        instance,
        region,
        frame,
        size,
        visible,
        scale_factor,
        monitors,
    );
    if let Some(view) = view {
        runtime.instances.set_view(instance, view);
    }
    runtime
        .instances
        .place(instance, region, Placement { rect, clip });
}

fn region_monitors(screens: &[beui::Screen], rect: Rect, size: Vec2) -> Vec<Monitor> {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return Vec::new();
    }
    let scale = vec2(size.x / rect.width(), size.y / rect.height());
    screens
        .iter()
        .filter(|screen| screen.rect.intersects(rect))
        .map(|screen| {
            let shown = screen.rect.translate(-rect.min.to_vec2());
            Monitor {
                id: screen.id.clone(),
                name: screen.name.clone(),
                rect: Rect::from_min_size(
                    Pos2::new(shown.min.x * scale.x, shown.min.y * scale.y),
                    Vec2::new(shown.width() * scale.x, shown.height() * scale.y),
                ),
            }
        })
        .collect()
}

pub(crate) fn forward_region(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    input: &beui::ForwardedInput,
) {
    with(plugin_id, |runtime| {
        let (messages, revoked) = runtime.instances.forward(instance, region, input);
        runtime.pacing.needed |= !messages.is_empty();
        runtime.send(messages);
        if revoked {
            mark(plugin_id);
        }
    });
}

pub(crate) fn pressed_outside_region(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
) {
    with(plugin_id, |runtime| {
        if runtime.instances.revoke_active(instance, region) {
            mark(plugin_id);
        }
    });
}

pub(crate) fn back_region(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    gesture: beui::BackGesture,
) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.back(instance, region, gesture);
        runtime.pacing.needed |= !messages.is_empty();
        runtime.send(messages);
    });
    host::request_repaint();
}

pub(crate) fn intercept_region(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    press: Option<beui::KeyPress>,
    modifiers: beui::Modifiers,
) {
    with(plugin_id, |runtime| {
        let messages = runtime
            .instances
            .intercepted(instance, region, press, modifiers);
        runtime.pacing.needed |= !messages.is_empty();
        runtime.send(messages);
    });
    host::request_repaint();
}

pub(crate) fn intercept_tap(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    key: beui::Key,
) {
    with(plugin_id, |runtime| {
        let messages = runtime.instances.intercepted_tap(instance, region, key);
        runtime.pacing.needed |= !messages.is_empty();
        runtime.send(messages);
    });
    host::request_repaint();
}

#[derive(Clone, PartialEq)]
pub(crate) struct RegionView {
    pub(crate) error: Option<(String, bool)>,
    pub(crate) rect: Rect,
    pub(crate) clip: Rect,
    pub(crate) loading: bool,
    pub(crate) base: Vec<Piece>,
    pub(crate) floating: Vec<Piece>,
    pub(crate) floating_rects: Vec<Rect>,
    pub(crate) drawing: Vec<Rect>,
    pub(crate) claims: Vec<(beui::Modifiers, Rect)>,
    pub(crate) held: Option<Rect>,
    pub(crate) drawn: Option<(u32, u32)>,
    pub(crate) children: Vec<HostChild>,
    pub(crate) holes: Vec<(Rect, Vec<Rect>)>,
    pub(crate) cursor: Option<beui::CursorIcon>,
    pub(crate) ime: Option<beui::ImeArea>,
    pub(crate) handles_back: bool,
    pub(crate) wants_keyboard: bool,
    pub(crate) keyboard_toggles: u32,
    pub(crate) intercepted_keys: Vec<beui::KeyChord>,
    pub(crate) pastes: Vec<beui::PasteKind>,
}

impl RegionView {
    fn failed(error: String, restart: bool) -> Self {
        Self {
            error: Some((error, restart)),
            rect: Rect::ZERO,
            clip: Rect::ZERO,
            loading: false,
            base: Vec::new(),
            floating: Vec::new(),
            floating_rects: Vec::new(),
            drawing: Vec::new(),
            claims: Vec::new(),
            held: None,
            drawn: None,
            children: Vec::new(),
            holes: Vec::new(),
            cursor: None,
            ime: None,
            handles_back: false,
            wants_keyboard: false,
            keyboard_toggles: 0,
            intercepted_keys: Vec::new(),
            pastes: Vec::new(),
        }
    }

    fn waiting() -> Self {
        Self {
            error: None,
            loading: true,
            ..Self::failed(String::new(), false)
        }
    }

    pub(crate) fn claims(&self, position: Pos2, held: beui::Modifiers) -> bool {
        self.claims.iter().any(|(claimed, rect)| {
            claimed.any() && held.holds(*claimed) && rect.contains_half_open(position)
        })
    }

    pub(crate) fn draws(&self, position: Pos2) -> bool {
        self.drawing
            .iter()
            .any(|rect| rect.contains_half_open(position))
            && !self
                .floating_rects
                .iter()
                .any(|rect| rect.contains_half_open(position))
    }

    pub(crate) fn takes(&self, local: Pos2) -> bool {
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

pub(crate) fn region_view(
    plugin: &PluginManifest,
    instance: EditorInstanceId,
    region: EditorRegion,
    own: Option<EmbedPlacement>,
    view: Option<EditorView>,
) -> RegionView {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let runtime = match host.runtime(plugin) {
            Ok(runtime) => runtime,
            Err(error) => return RegionView::failed(error, false),
        };
        let view = runtime.contain(|runtime| {
            runtime
                .error
                .is_none()
                .then(|| placed_view(runtime, instance, region, own, view))
        });
        match (view, runtime.error.clone()) {
            (_, Some(error)) => RegionView::failed(error, true),
            (Some(view), None) => view,
            (None, None) => RegionView::waiting(),
        }
    })
}

fn placed_view(
    runtime: &mut Runtime,
    instance: EditorInstanceId,
    region: EditorRegion,
    own: Option<EmbedPlacement>,
    view: Option<EditorView>,
) -> RegionView {
    {
        if let Some(stopped) = runtime.instances.stopped(instance, region) {
            let error = match stopped {
                block_plugin_api::Stopped::Closed => "The editor closed".to_owned(),
                block_plugin_api::Stopped::Failed(failure) => {
                    format!("The editor stopped after a panic: {failure}")
                }
            };
            return RegionView::failed(error, true);
        }
        let Some(placement) = runtime.instances.placement(instance, region) else {
            return RegionView::waiting();
        };
        let chosen = (placement.rect, placement.clip);
        let (rect, clip) = own.map_or(chosen, |own| (own.rect, own.clip));
        let screen = runtime.instances.screen_id(instance, region);
        let drawn = screen
            .and_then(|screen| runtime.layout.placement(screen))
            .map(|placement| (placement.width, placement.height));
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
            true => runtime
                .instances
                .held(instance, region, Some(visible), clip, drawn),
            false => None,
        };
        let (children, holes) = runtime
            .instances
            .host_children(instance, region, rect, clip, view);
        let report = runtime.instances.frame_report(instance, region);
        let floating_rects: Vec<Rect> = match held {
            Some(_) => Vec::new(),
            None => report
                .map(|report| {
                    report
                        .floating
                        .iter()
                        .map(|floating| floating.translate(rect.min.to_vec2()))
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
        let base: Vec<Rect> = match floating_rects.is_empty() {
            true => vec![visible],
            false => super::pieces::subtract(visible, &floating_rects),
        };
        let (base, floating) = match held {
            Some(_) => (
                vec![Piece {
                    local: UNIT,
                    source: UNIT,
                }],
                Vec::new(),
            ),
            None => (
                pieces_of(rect, visible, textured, &base),
                pieces_of(rect, visible, textured, &floating_rects),
            ),
        };
        let loading = screen.is_none_or(|screen| runtime.layout.placement(screen).is_none());
        RegionView {
            error: None,
            rect,
            clip,
            loading,
            base,
            floating,
            floating_rects,
            drawing,
            claims,
            held: held.map(|held| held.rect),
            drawn: held.map(|held| held.drawn),
            children,
            holes: holes.parts(),
            cursor: match runtime.instances.dragging(instance, region)
                && runtime.instances.drag_accepted(instance)
            {
                true => Some(beui::CursorIcon::Alias),
                false => runtime.instances.cursor(instance, region),
            },
            ime: runtime.instances.ime(instance, region, rect),
            handles_back,
            wants_keyboard: window.is_some_and(|window| window.wants_keyboard),
            keyboard_toggles: window.map_or(0, |window| window.keyboard_toggles),
            intercepted_keys,
            pastes: window
                .map(|window| window.pastes.clone())
                .unwrap_or_default(),
        }
    }
}

pub(crate) fn frames(plugin_id: &str) -> u64 {
    with(plugin_id, |runtime| runtime.frames).unwrap_or_default()
}

pub(crate) fn take_focus_request(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
) -> bool {
    with(plugin_id, |runtime| {
        runtime.instances.take_focus_request(instance, region)
    })
    .unwrap_or(false)
}

pub(crate) fn take_region_actions(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Vec<crate::editors::EditorAction> {
    use crate::editors::EditorAction;
    with(plugin_id, |runtime| {
        let mut actions = Vec::new();
        while let Some(open) = runtime.instances.take_open(instance) {
            actions.push(EditorAction::OpenBlock(open));
        }
        while let Some((id, command)) = runtime.instances.take_block_command(instance) {
            actions.push(match command {
                block_plugin_api::BlockCommand::AppMenu { anchor } => EditorAction::AppMenu {
                    at: anchor.and_then(|anchor| in_host(runtime, instance, anchor)),
                },
                command => EditorAction::Command { id, command },
            });
        }
        actions
    })
    .unwrap_or_default()
}

fn in_host(runtime: &Runtime, instance: EditorInstanceId, anchor: Rect) -> Option<Rect> {
    let placed = runtime.placements.get(&(instance, EditorRegion::Frame))?;
    let rect = Rect::from_min_size(
        placed.rect.min + anchor.min.to_vec2() * placed.zoom,
        anchor.size().max(Vec2::ZERO) * placed.zoom,
    );
    [rect.min.x, rect.min.y, rect.max.x, rect.max.y]
        .iter()
        .all(|value| value.is_finite())
        .then_some(rect)
}

pub(crate) fn take_block_drags(plugin_id: &str, instance: EditorInstanceId) -> Vec<BlockDrag> {
    with(plugin_id, |runtime| {
        runtime.instances.take_block_drags(instance)
    })
    .unwrap_or_default()
}

pub(crate) fn drag_over_region(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    over: Option<(BlockDrag, Pos2)>,
    dropped: bool,
) {
    with(plugin_id, |runtime| {
        let origin = runtime
            .instances
            .placement(instance, region)
            .map_or(Pos2::ZERO, |placement| placement.rect.min);
        let was = runtime.instances.dragging(instance, region);
        let event = over.map(|(drag, at)| BlockDragEvent {
            position: Pos2::ZERO + (at - origin),
            drag,
            dropped,
        });
        let messages = runtime.instances.drag(instance, region, event);
        runtime.pacing.needed |= !messages.is_empty();
        runtime.send(messages);
        if was != runtime.instances.dragging(instance, region) {
            mark(plugin_id);
        }
    });
}

pub(crate) fn region_drawing(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    view: &RegionView,
    pieces: &[Piece],
    rotation: f32,
    opacity: f32,
) -> Option<beui::Drawing> {
    if pieces.is_empty() || view.loading {
        return None;
    }
    with(plugin_id, |runtime| {
        let screen = runtime.instances.screen_id(instance, region)?;
        let mut template = runtime.template(screen, view.drawn);
        template.quad.opacity = opacity;
        Some(beui::painted_drawing(RegionDrawing::new(
            template,
            pieces.to_vec(),
            view.held,
            rotation,
        )))
    })
    .flatten()
}

pub(crate) fn region_placed(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
) -> Option<[u32; 3]> {
    with(plugin_id, |runtime| {
        let screen = runtime.instances.screen_id(instance, region)?;
        runtime
            .layout
            .placement(screen)
            .map(|placement| [placement.surface, placement.width, placement.height])
    })
    .flatten()
}

pub(crate) fn region_damage(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    view: &RegionView,
    pieces: &[Piece],
    rotation: f32,
    seen: u64,
) -> Option<Vec<Rect>> {
    if view.held.is_some() || rotation != 0.0 {
        return None;
    }
    with(plugin_id, |runtime| {
        let screen = runtime.instances.screen_id(instance, region)?;
        runtime.template(screen, view.drawn).damage(pieces, seen)
    })
    .flatten()
}

pub(crate) fn region_published(plugin_id: &str) -> u64 {
    with(plugin_id, |runtime| {
        runtime.shared.borrow().published.count()
    })
    .unwrap_or_default()
}

pub(super) fn each(mut act: impl FnMut(&mut Runtime)) {
    HOST.with(|host| host.borrow_mut().runtimes.values_mut().for_each(&mut act));
}

pub(super) fn with<R>(plugin_id: &str, act: impl FnOnce(&mut Runtime) -> R) -> Option<R> {
    HOST.with(|host| {
        host.borrow_mut()
            .runtimes
            .get_mut(plugin_id)?
            .contain(|runtime| Some(act(runtime)))
    })
}

#[cfg(test)]
mod tests;
