use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use beui::{Pos2, Rect, Vec2, pos2, vec2};
use block_plugin_api::{
    ArtifactDescription, BlockPick, DEFAULT_SURFACE_SIDE, EditorInstanceId, EditorMessage,
    EditorRegion, FrameSpec, HostPanel, HostSession, MAX_QUEUED_MESSAGES, Message, PluginManifest,
    PresentedFrame, ScreenId, ScreenLayout, ScreenRequest, SessionState, SurfaceFormat,
    SurfaceRect, SurfaceSpec, Theme, ViewChange,
};
use uuid::Uuid;

use crate::host;

use super::{
    ArtifactSlot, ArtifactState, BlockPickRequest, CreationSlot, CreationState, HostChild,
    HostChildStatus, InstanceRole, RuntimeStatus, SurfaceStatus,
    backend::{Availability, Backend, Deadline, Platform, ShownFrame},
    instances::{EditorView, Focus, Instances, Placement},
    presenter::{
        self, Blit, MAX_SURFACES, Piece, PresenterState, PresenterStatus, Quad, RegionDrawing,
        Shared,
    },
    preview_size,
};

const CROWDED: &str = "Too many plugin runtimes are already presenting.";
const HOST_NAME: &str = "BE3";
const UNIT: Rect = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
const FRAME_TIMEOUT_SECONDS: f64 = 1.0;
const FRAME_BUDGET: Duration = Duration::from_millis(8);
pub(crate) const PACING: &str = "Frame pacing";
const REMEMBERED_PRESENTS: usize = 16;
const SURFACE: SurfaceSpec = SurfaceSpec {
    format: SurfaceFormat::Rgba8Unorm,
    max_side: DEFAULT_SURFACE_SIDE,
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
            grabbed: false,
        }
    }

    fn runtime(&mut self, plugin: &PluginManifest) -> Result<&mut Runtime, String> {
        if let Err(error) = &self.availability.0 {
            return Err(error.clone());
        }
        let Some(surface) = self.surface_for(&plugin.identity.id) else {
            return Err(CROWDED.to_owned());
        };
        let focus = self.focus.clone();
        let runtime = self
            .runtimes
            .entry(plugin.identity.id.clone())
            .or_insert_with(|| Runtime::new(plugin, surface));
        runtime.instances.set_focus(focus);
        runtime.begin_pass(host::pass());
        Ok(runtime)
    }

    fn surface_for(&mut self, plugin_id: &str) -> Option<u32> {
        if let Some(runtime) = self.runtimes.get(plugin_id) {
            return Some(runtime.surface);
        }
        if let Some(surface) = (0..MAX_SURFACES).find(|surface| {
            !self
                .runtimes
                .values()
                .any(|runtime| runtime.surface == *surface)
        }) {
            return Some(surface);
        }
        let evicted = self
            .runtimes
            .iter()
            .filter(|(_, runtime)| runtime.instances.is_empty())
            .min_by_key(|(_, runtime)| runtime.pass)
            .map(|(id, _)| id.clone())?;
        self.shutdown(&evicted)
    }

    fn shutdown(&mut self, plugin_id: &str) -> Option<u32> {
        let mut runtime = self.runtimes.remove(plugin_id)?;
        runtime.stop();
        presenter::release(runtime.surface, &runtime.status);
        host::request_repaint();
        Some(runtime.surface)
    }
}

pub(super) struct Runtime {
    plugin: PluginManifest,
    pub(super) backend: Platform,
    session: HostSession,
    queued: Vec<Message>,
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
    needed: bool,
    paint_at: Option<f64>,
    requested_at: Option<f64>,
    animated: u64,
    theme: Theme,
    fonts_sent: bool,
    fallbacks: super::fonts::Fallbacks,
    presents: Presents,
    frames: u64,
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

    fn damage_through(&mut self, presents: u64) -> Option<Vec<SurfaceRect>> {
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
        let mut backend = Platform::new(plugin);
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
            needed: false,
            paint_at: None,
            requested_at: None,
            animated: 0,
            theme: theme(),
            fonts_sent: false,
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
        self.error = None;
        self.status = PresenterStatus::waiting();
        self.layout = ScreenLayout::default();
        self.sent.clear();
        self.needed = false;
        self.paint_at = None;
        self.requested_at = None;
        self.theme = theme();
        self.fonts_sent = false;
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
        let theme = theme();
        if self.theme != theme {
            self.theme = theme;
            messages.push(Message::Theme(theme));
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
        let awaited = messages.iter().any(|message| {
            matches!(
                message,
                Message::Editor(EditorMessage::Open { .. } | EditorMessage::OpenArtifact { .. })
            )
        });
        self.needed |= !messages.is_empty();
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
        let messages = self.instances.drive_web_views(self.pass);
        self.needed |= !messages.is_empty();
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

    fn request_frame(&mut self) {
        if self.error.is_some() {
            return;
        }
        if self.session.granted_surface().is_some() && self.frame_due() {
            self.requested_at = Some(self.now());
            self.animated = host::pass();
            self.send(vec![Message::DrawFrame]);
        }
        if self.requested_at.is_some() {
            host::request_repaint_after(Duration::from_secs_f64(FRAME_TIMEOUT_SECONDS));
        }
    }

    fn frame_due(&self) -> bool {
        let now = self.now();
        (self.needed
            || (self.animated != host::pass() && self.paint_at.is_some_and(|at| at <= now)))
            && self
                .requested_at
                .is_none_or(|at| now - at >= FRAME_TIMEOUT_SECONDS)
    }

    fn now(&self) -> f64 {
        host::now()
    }

    fn milliseconds(&self) -> u64 {
        host::milliseconds()
    }

    fn detect_error(&mut self) {
        if self.error.is_some() {
            return;
        }
        self.error = self.backend.take_error().or(match self.status.get() {
            PresenterState::Failed(error) | PresenterState::Unsupported(error) => Some(error),
            PresenterState::Waiting | PresenterState::Presenting | PresenterState::Released => None,
        });
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
            state => {
                self.error = Some(format!("The plugin session stopped: {state:?}"));
                return;
            }
        }
        self.apply(forwarded);
        if let Some(mut frame) = self.backend.received_frame() {
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

    pub(super) fn apply(&mut self, messages: Vec<Message>) {
        if messages.is_empty() {
            return;
        }
        let mut answers = Vec::new();
        let mut changed = false;
        mark(&self.plugin.identity.id);
        for mut message in messages {
            self.instances.translate(&mut message, true);
            changed |= match message {
                Message::Layout(layout) => {
                    self.layout = layout;
                    true
                }
                Message::Editor(message) => self.instances.editor_message(message),
                Message::FrameNeeded => {
                    self.needed = true;
                    true
                }
                Message::FrameReady(frame) => {
                    self.await_next_frame(frame.repaint_after_micros);
                    if let Some(presented) = frame.presented {
                        self.presents.report(presented);
                    }
                    false
                }
                Message::RegionSizes(sizes) => self.instances.set_region_sizes(sizes),
                Message::MissingCharacters(missing) => {
                    answers.extend(self.fallbacks.answer(&missing).map(Message::Fonts));
                    false
                }
                Message::Frames(reports) => self.instances.set_frame_reports(reports),
                Message::Children(placements) => {
                    let (answered, changed) = self.instances.set_children(placements);
                    answers.extend(answered);
                    changed
                }
                _ => false,
            };
        }
        self.send(answers);
        if changed {
            host::request_repaint();
        }
    }

    fn await_next_frame(&mut self, repaint_after_micros: Option<u64>) {
        self.needed = false;
        self.requested_at = None;
        self.paint_at = repaint_after_micros.map(|micros| {
            let delay = Duration::from_micros(micros);
            host::request_repaint_after(delay);
            self.now() + delay.as_secs_f64()
        });
    }

    fn send(&mut self, mut messages: Vec<Message>) {
        if messages.is_empty() || self.error.is_some() {
            return;
        }
        for message in &mut messages {
            self.instances.translate(message, false);
        }
        self.queued.extend(self.instances.gate(messages));
        self.deliver();
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
        let frame = self.backend.frame(&self.layout, self.pass);
        if frame.is_some() {
            self.frames += 1;
            mark(&self.plugin.identity.id);
        }
        self.shared.borrow_mut().publish(&self.layout, frame);
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
                .map(|placement| [placement.x, placement.y, placement.width, placement.height]),
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

pub(crate) struct HostFrame {
    pub(crate) content: Rect,
}

pub(crate) fn report_child_views(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    changes: Vec<(block_plugin_api::ChildId, ViewChange)>,
) {
    if changes.is_empty() {
        return;
    }
    with(plugin_id, |runtime| {
        let messages = runtime
            .instances
            .child_view_changes(instance, region, changes);
        runtime.send(messages);
    });
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

pub(crate) fn request_pick(
    plugin_id: &str,
    instance: EditorInstanceId,
    pick: u64,
    filter: block_plugin_api::BlockFilter,
    parent: block_plugin_api::BlockLocation,
) {
    with(plugin_id, |runtime| {
        let messages = runtime
            .instances
            .request_pick(instance, pick, filter, parent);
        runtime.send(messages);
    });
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
        if let Some(error) = runtime.error.clone() {
            return CreationState::Failed(error);
        }
        match runtime
            .instances
            .report_creation(instance, client_id, block_types, role)
        {
            true => CreationState::Ready,
            false => CreationState::Starting,
        }
    })
}

pub(crate) fn artifact(slot: ArtifactSlot<'_>) -> ArtifactState {
    let ArtifactSlot {
        plugin,
        block_types,
        client_id,
        instance,
        source_type,
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
        if let Some(error) = runtime.error.clone() {
            return ArtifactState::Failed(error);
        }
        let messages = runtime.instances.report_artifact(
            instance,
            client_id,
            block_types,
            source_type,
            block,
            data,
            resync,
        );
        runtime.send(messages);
        match runtime.instances.artifact_description(instance) {
            Some(ArtifactDescription::Described { source, summary }) => ArtifactState::Described {
                source: Uuid::from_bytes(source),
                summary,
            },
            Some(ArtifactDescription::Unreadable(error)) => ArtifactState::Failed(error),
            None => ArtifactState::Starting,
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
        let pressed = host::pressed_at();
        for runtime in host.runtimes.values_mut() {
            if let Some(position) = pressed
                && runtime.instances.pressed_at(position)
            {
                mark(&runtime.plugin.identity.id);
            }
            runtime.instances.touch(&touched);
            runtime.detect_error();
            runtime.pump();
            runtime.begin_frame(pass);
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
            _ => Deadline::after(FRAME_BUDGET),
        };
        host.deadline = Some((pass, deadline));
        let asked: Vec<String> = host
            .runtimes
            .iter_mut()
            .filter_map(|(id, runtime)| runtime.ask(pass).then(|| id.clone()))
            .collect();
        let started = std::time::Instant::now();
        let mut over = false;
        for id in asked {
            if let Some(runtime) = host.runtimes.get_mut(&id) {
                over |= runtime.settle(deadline);
            }
        }
        host.waited += started.elapsed();
        host.over_budget += u64::from(over);
    });
}

pub(crate) fn start_frames() {
    let pass = host::pass();
    HOST.with(|host| {
        for runtime in host.borrow_mut().runtimes.values_mut() {
            runtime.start(pass);
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
            runtime.flush();
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
                runtime.needed = true;
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

pub(crate) fn take_closed_windows(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Vec<block_plugin_api::HostWindowId> {
    with(plugin_id, |runtime| {
        runtime.instances.take_closed_windows(instance)
    })
    .unwrap_or_default()
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
    plugin_id: &str,
    instance: EditorInstanceId,
    block_id: Uuid,
    block_type: Uuid,
    via: Option<Uuid>,
) {
    with(plugin_id, |runtime| {
        let messages = runtime
            .instances
            .show_block(instance, block_id, block_type, via);
        runtime.send(messages);
    });
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
            .map(|report| {
                let rect = |rect: &block_plugin_api::ChildRect| {
                    Rect::from_min_size(pos2(rect.x, rect.y), vec2(rect.width, rect.height))
                };
                HostFrame {
                    content: rect(&report.content),
                }
            })
    })
    .flatten()
}

pub(crate) fn presenting(plugin_id: &str, instance: EditorInstanceId) -> bool {
    with(plugin_id, |runtime| runtime.instances.presenting(instance)).unwrap_or_default()
}

pub(crate) fn set_windows(
    plugin_id: &str,
    instance: EditorInstanceId,
    windows: Vec<block_plugin_api::HostWindow>,
) {
    with(plugin_id, |runtime| {
        if runtime.instances.set_windows(instance, windows) {
            mark(plugin_id);
            host::request_repaint();
        }
    });
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

pub(crate) fn intrinsic_size(plugin_id: &str, instance: EditorInstanceId) -> Option<Vec2> {
    with(plugin_id, |runtime| {
        runtime.instances.intrinsic_size(instance)
    })
    .flatten()
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
                    generation: runtime.layout.generation,
                    width: runtime.layout.width,
                    height: runtime.layout.height,
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
    HostSession::new(HOST_NAME, Some(SURFACE), theme())
}

fn theme() -> Theme {
    Theme { dark: host::dark() }
}

thread_local! {
    static CHANGED: RefCell<std::collections::HashSet<String>> =
        RefCell::new(std::collections::HashSet::new());
}

fn mark(plugin_id: &str) {
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
        runtime
            .instances
            .mount(instance, region, client_id, role, block_types);
        mark(&plugin.identity.id);
    });
    host::request_repaint();
}

pub(crate) fn unmount_region(plugin_id: &str, instance: EditorInstanceId, region: EditorRegion) {
    with(plugin_id, |runtime| {
        runtime.instances.unmount(instance, region)
    });
    mark(plugin_id);
    host::request_repaint();
}

pub(crate) fn unplace_region(plugin_id: &str, instance: EditorInstanceId, region: EditorRegion) {
    with(plugin_id, |runtime| {
        runtime.instances.unplace(instance, region)
    });
    mark(plugin_id);
    host::request_repaint();
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RegionPlacement {
    pub(crate) rect: Rect,
    pub(crate) clip: Rect,
}

pub(crate) fn place_region(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    placement: RegionPlacement,
    frame: Option<FrameSpec>,
    view: Option<EditorView>,
) {
    with(plugin_id, |runtime| {
        if !runtime.instances.mounted(instance, region) {
            return;
        }
        runtime.placed = host::pass();
        let RegionPlacement { rect, clip } = placement;
        let scale_factor = host::pixels_per_point();
        let size = match region {
            EditorRegion::Preview => preview_size(rect.size(), scale_factor),
            _ => rect.size(),
        };
        let cropped = Quad::upright(rect).crop_to(clip);
        let visible = cropped
            .as_ref()
            .map_or(Rect::ZERO, |(_, source)| scale_rect(*source, size));
        runtime
            .instances
            .place_mounted(instance, region, frame, size, visible, scale_factor);
        if let Some(view) = view {
            runtime.instances.set_view(instance, view);
        }
        runtime.instances.place(
            instance,
            region,
            Placement {
                rect,
                clip,
                pass: runtime.pass,
            },
        );
        mark(plugin_id);
    });
    host::request_repaint();
}

pub(crate) fn forward_region(
    plugin_id: &str,
    instance: EditorInstanceId,
    region: EditorRegion,
    input: &beui::ForwardedInput,
) {
    with(plugin_id, |runtime| {
        let (messages, revoked) = runtime.instances.forward(instance, region, input);
        runtime.needed |= !messages.is_empty();
        runtime.send(messages);
        if revoked {
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
        runtime.needed |= !messages.is_empty();
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
    pub(crate) held: Option<Rect>,
    pub(crate) drawn: Option<(u32, u32)>,
    pub(crate) children: Vec<HostChild>,
    pub(crate) holes: Vec<(Rect, Vec<Rect>)>,
    pub(crate) cursor: Option<beui::CursorIcon>,
    pub(crate) ime: Option<beui::ImeArea>,
    pub(crate) handles_back: bool,
    pub(crate) grabbed: bool,
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
            held: None,
            drawn: None,
            children: Vec::new(),
            holes: Vec::new(),
            cursor: None,
            ime: None,
            handles_back: false,
            grabbed: false,
        }
    }

    fn waiting() -> Self {
        Self {
            error: None,
            loading: true,
            ..Self::failed(String::new(), false)
        }
    }

    pub(crate) fn takes(&self, local: Pos2) -> bool {
        let position = local + self.rect.min.to_vec2();
        !self.holes.iter().any(|(hole, occluders)| {
            hole.contains(position) && !occluders.iter().any(|rect| rect.contains(position))
        })
    }
}

fn pieces_of(rect: Rect, visible: Rect, shown: &[Rect]) -> Vec<Piece> {
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
        .map(|piece| piece.intersect(visible))
        .filter(|piece| piece.is_positive())
        .map(|piece| Piece {
            local: fraction(piece, rect),
            source: fraction(piece, visible),
        })
        .collect()
}

pub(crate) fn region_view(
    plugin: &PluginManifest,
    instance: EditorInstanceId,
    region: EditorRegion,
) -> RegionView {
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let runtime = match host.runtime(plugin) {
            Ok(runtime) => runtime,
            Err(error) => return RegionView::failed(error, false),
        };
        if let Some(error) = runtime.error.clone() {
            return RegionView::failed(error, true);
        }
        let Some(placement) = runtime.instances.placement(instance, region) else {
            return RegionView::waiting();
        };
        let (rect, clip) = (placement.rect, placement.clip);
        let screen = runtime.instances.screen_id(instance, region);
        let drawn = screen
            .and_then(|screen| runtime.layout.placement(screen))
            .map(|placement| (placement.width, placement.height));
        let visible = rect.intersect(clip);
        let held = runtime
            .instances
            .held(instance, region, Some(visible), clip, drawn);
        let (children, holes) = runtime
            .instances
            .host_children(instance, region, rect, clip);
        let report = runtime.instances.frame_report(instance, region);
        let floating_rects: Vec<Rect> = match held {
            Some(_) => Vec::new(),
            None => report
                .map(|report| {
                    report
                        .floating
                        .iter()
                        .map(|floating| {
                            Rect::from_min_size(
                                pos2(floating.x, floating.y) + rect.min.to_vec2(),
                                vec2(floating.width, floating.height),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
        };
        let handles_back = report.is_some_and(|report| report.handles_back);
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
                pieces_of(rect, visible, &base),
                pieces_of(rect, visible, &floating_rects),
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
            grabbed: runtime.instances.grabbing(),
        }
    })
}

pub(crate) fn frames(plugin_id: &str) -> u64 {
    with(plugin_id, |runtime| runtime.frames).unwrap_or_default()
}

pub(crate) fn take_region_actions(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Vec<crate::editors::EditorAction> {
    use crate::editors::EditorAction;
    with(plugin_id, |runtime| {
        let mut actions = Vec::new();
        while let Some((id, block_type, via)) = runtime.instances.take_open(instance) {
            actions.push(EditorAction::OpenBlock {
                id,
                block_type,
                via,
            });
        }
        while let Some((id, block_type)) = runtime.instances.take_block_drag(instance) {
            actions.push(EditorAction::DragBlock { id, block_type });
        }
        while let Some((id, command)) = runtime.instances.take_block_command(instance) {
            actions.push(EditorAction::Command { id, command });
        }
        actions
    })
    .unwrap_or_default()
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
        Some(beui::drawing(RegionDrawing::new(
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
) -> Option<[u32; 4]> {
    with(plugin_id, |runtime| {
        let screen = runtime.instances.screen_id(instance, region)?;
        runtime
            .layout
            .placement(screen)
            .map(|placement| [placement.x, placement.y, placement.width, placement.height])
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
) -> Option<Vec<Rect>> {
    if view.held.is_some() || rotation != 0.0 {
        return None;
    }
    with(plugin_id, |runtime| {
        let screen = runtime.instances.screen_id(instance, region)?;
        runtime.template(screen, view.drawn).damage(pieces)
    })
    .flatten()
}

pub(super) fn with<R>(plugin_id: &str, act: impl FnOnce(&mut Runtime) -> R) -> Option<R> {
    HOST.with(|host| Some(act(host.borrow_mut().runtimes.get_mut(plugin_id)?)))
}

#[cfg(test)]
mod tests;
