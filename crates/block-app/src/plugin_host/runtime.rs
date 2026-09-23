use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use block_plugin_api::{
    ArtifactDescription, BlockCommand, BlockPick, DEFAULT_SURFACE_SIDE, EditorInstanceId,
    EditorMessage, EditorRegion, HostSession, MAX_QUEUED_MESSAGES, Message, PluginManifest,
    ScreenId, ScreenLayout, ScreenRequest, SessionState, SurfaceFormat, SurfaceSpec, Theme,
    ViewChange,
};
use beui::{Pos2, Rect, Vec2, pos2, vec2};
use uuid::Uuid;

use crate::host::{self, HostItem, Target, Ui};

use super::{
    ArtifactSlot, ArtifactState, BlockPickRequest, CreationSlot, CreationState, EditorBlock,
    EditorSlot, HostChild, HostChildStatus, InstanceRole, PreviewPresentation, PreviewSlot,
    RuntimeStatus, SurfaceStatus,
    backend::{Availability, Backend, Platform},
    input,
    instances::{Focus, FrameOverlay, Instances, OpenRequest, Placement},
    presenter::{self, Blit, MAX_SURFACES, PresenterState, PresenterStatus, Quad, Shared},
    preview_size,
};

const CROWDED: &str = "Too many plugin runtimes are already presenting.";
const HOST_NAME: &str = "BE3";
const UNIT: Rect = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
const FRAME_TIMEOUT_SECONDS: f64 = 1.0;
const SURFACE: SurfaceSpec = SurfaceSpec {
    format: SurfaceFormat::Rgba8Unorm,
    max_side: DEFAULT_SURFACE_SIDE,
};

thread_local! {
    static HOST: RefCell<Host> = RefCell::new(Host::new());
}

struct Host {
    availability: Availability,
    runtimes: HashMap<String, Runtime>,
    focus: Focus,
    grabbed: bool,
    overlay: FrameOverlay,
}

impl Host {
    fn new() -> Self {
        Self {
            availability: Availability::missing(),
            runtimes: HashMap::new(),
            focus: Focus::default(),
            grabbed: false,
            overlay: FrameOverlay::default(),
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
    surface: u32,
    status: PresenterStatus,
    shared: Arc<Mutex<Shared>>,
    presented: bool,
    sent: Vec<ScreenRequest>,
    error: Option<String>,
    needed: bool,
    paint_at: Option<f64>,
    requested_at: Option<f64>,
    theme: Theme,
}

impl Runtime {
    fn new(plugin: &PluginManifest, surface: u32) -> Self {
        let mut backend = Platform::new(plugin);
        backend.start(plugin);
        let mut instances = Instances::default();
        instances.allow_network(plugin.network.clone());
        let mut session = session();
        session.start(host::milliseconds());
        Self {
            plugin: plugin.clone(),
            backend,
            session,
            queued: Vec::new(),
            instances,
            layout: ScreenLayout::default(),
            pass: 0,
            surface,
            status: PresenterStatus::waiting(),
            shared: Arc::new(Mutex::new(Shared::default())),
            presented: false,
            sent: Vec::new(),
            error: None,
            needed: false,
            paint_at: None,
            requested_at: None,
            theme: theme(),
        }
    }

    fn stop(&mut self) {
        self.session.shutdown(self.milliseconds());
        self.drain();
        self.backend.shutdown();
    }

    fn restart(&mut self) {
        let plugin = self.plugin.clone();
        self.stop();
        self.session = session();
        self.session.start(self.milliseconds());
        self.queued.clear();
        self.error = None;
        self.status = PresenterStatus::waiting();
        self.layout = ScreenLayout::default();
        self.sent.clear();
        self.needed = false;
        self.paint_at = None;
        self.requested_at = None;
        self.theme = theme();
        self.instances.reopen();
        self.backend.start(&plugin);
    }

    fn begin_pass(&mut self, pass: u64) {
        self.detect_error();
        if self.pass == pass || self.error.is_some() {
            return;
        }
        let previous = self.pass;
        self.pass = pass;
        let drawing = self.session.granted_surface().is_some();
        let next = self.instances.next_screens(previous);
        let mut messages = Vec::new();
        let theme = theme();
        if self.theme != theme {
            self.theme = theme;
            messages.push(Message::Theme(theme));
        }
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

    fn begin_frame(&mut self, pass: u64, overlay: &FrameOverlay) {
        if self.error.is_some() || self.pass + 1 < pass {
            return;
        }
        let mut messages = match self.pass + 1 == pass {
            true => self.instances.frame_input(self.pass, overlay),
            false => Vec::new(),
        };
        messages.extend(self.instances.drive_web_views(self.pass));
        self.needed |= !messages.is_empty();
        if self.session.granted_surface().is_some() && self.frame_due() {
            self.requested_at = Some(self.now());
            messages.push(Message::DrawFrame);
        }
        self.send(messages);
    }

    fn frame_due(&self) -> bool {
        let now = self.now();
        (self.needed || self.paint_at.is_some_and(|at| at <= now))
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
    }

    fn pump(&mut self) {
        if self.error.is_some() {
            return;
        }
        let now = self.milliseconds();
        let received = self.backend.receive();
        let mut forwarded = Vec::with_capacity(received.len());
        for message in received {
            match message.is_session() {
                true => self.session.receive(message, now),
                false => forwarded.push(message),
            }
        }
        self.deliver();
        self.session.tick(now);
        match self.session.state() {
            SessionState::Idle | SessionState::Starting | SessionState::Running => {}
            state => {
                self.error = Some(format!("The plugin session stopped: {state:?}"));
                return;
            }
        }
        self.apply(forwarded);
        let responses = self.instances.client_responses();
        self.send(responses);
    }

    pub(super) fn apply(&mut self, messages: Vec<Message>) {
        if messages.is_empty() {
            return;
        }
        let mut answers = Vec::new();
        let mut changed = false;
        for message in messages {
            changed |= match message {
                Message::Layout(layout) => {
                    self.layout = layout;
                    true
                }
                Message::Client(message) => {
                    self.instances.client_message(message);
                    false
                }
                Message::Editor(message) => self.instances.editor_message(message),
                Message::FrameNeeded => {
                    self.needed = true;
                    true
                }
                Message::FrameReady(frame) => {
                    self.await_next_frame(frame.repaint_after_micros);
                    false
                }
                Message::RegionSizes(sizes) => self.instances.set_region_sizes(sizes),
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

    fn send(&mut self, messages: Vec<Message>) {
        if messages.is_empty() || self.error.is_some() {
            return;
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

    fn present(
        &mut self,
        screen: ScreenId,
        quad: Quad,
        source: Rect,
        drawn: Option<(u32, u32)>,
    ) -> Blit {
        self.presented = true;
        Blit {
            surface: self.surface,
            status: self.status.clone(),
            shared: Arc::clone(&self.shared),
            screen,
            quad,
            source,
            drawn,
        }
    }

    fn flush(&mut self) {
        self.pump();
        if !std::mem::take(&mut self.presented) {
            return;
        }
        let frame = self.backend.frame(&self.layout, self.pass);
        self.shared.lock().unwrap().publish(&self.layout, frame);
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

pub(crate) fn install(setup: &beui::Setup) {
    let availability = presenter::install(setup);
    HOST.with(|host| {
        host.borrow_mut().availability = availability;
    });
}

fn plugin_loading_rect(
    layout: &ScreenLayout,
    screen: ScreenId,
    rect: Rect,
) -> Option<Rect> {
    layout.placement(screen).is_none().then_some(rect)
}

pub(crate) struct HostFrame {
    pub(crate) content: Rect,
}

pub(crate) struct EditorPresentation {
    plugin_id: String,
    instance: EditorInstanceId,
    region: EditorRegion,
    pub(crate) id: Option<Target>,
    pub(crate) loading_rect: Option<Rect>,
    screen: Option<ScreenId>,
    quad: Option<Quad>,
    clip: Rect,
    drawn: Option<(u32, u32)>,
    floating: Vec<Rect>,
    pub(crate) open: Option<OpenRequest>,
    pub(crate) drag: Option<(Uuid, Uuid)>,
    pub(crate) command: Option<(Uuid, BlockCommand)>,
    pub(crate) children: Vec<HostChild>,
}

impl EditorPresentation {
    fn empty(plugin_id: &str, instance: EditorInstanceId, region: EditorRegion) -> Self {
        Self {
            plugin_id: plugin_id.to_owned(),
            instance,
            region,
            id: None,
            loading_rect: None,
            screen: None,
            quad: None,
            clip: Rect::ZERO,
            drawn: None,
            floating: Vec::new(),
            open: None,
            drag: None,
            command: None,
            children: Vec::new(),
        }
    }

    fn blit(&self, ui: &mut Ui, rects: &[Rect]) {
        let (Some(screen), Some(quad)) = (self.screen, self.quad) else {
            return;
        };
        let base = quad.rect;
        if !base.is_positive() {
            return;
        }
        with(&self.plugin_id, |runtime| {
            for rect in rects {
                let piece = rect.intersect(base).intersect(self.clip);
                if !piece.is_positive() {
                    continue;
                }
                let source = Rect::from_min_max(
                    pos2(
                        (piece.min.x - base.min.x) / base.width(),
                        (piece.min.y - base.min.y) / base.height(),
                    ),
                    pos2(
                        (piece.max.x - base.min.x) / base.width(),
                        (piece.max.y - base.min.y) / base.height(),
                    ),
                );
                ui.blit(runtime.present(screen, Quad::upright(piece), source, self.drawn));
            }
        });
    }

    pub(crate) fn present(&self, ui: &mut Ui) {
        let Some(quad) = self.quad else {
            return;
        };
        let base = match self.floating.is_empty() {
            true => vec![quad.rect],
            false => super::pieces::subtract(quad.rect, &self.floating),
        };
        self.blit(ui, &base);
    }

    pub(crate) fn present_floating(&self, ui: &mut Ui) {
        if self.floating.is_empty() {
            return;
        }
        let floating = self.floating.clone();
        self.blit(ui, &floating);
    }

    pub(crate) fn report(&self, statuses: Vec<HostChildStatus>) {
        report_children(&self.plugin_id, self.instance, self.region, statuses);
    }
}

pub(crate) fn editor_ui(ui: &mut Ui, slot: EditorSlot<'_>) -> EditorPresentation {
    let EditorSlot {
        plugin,
        block_types,
        client,
        client_id,
        role,
        instance,
        region,
        frame,
        size,
        view,
    } = slot;
    let rect = Rect::from_min_size(ui.rect().min, size);
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let runtime = match host.runtime(plugin) {
            Ok(runtime) => runtime,
            Err(error) => {
                ui.item(
                    ("plugin-error", instance.0, region),
                    rect,
                    HostItem::Error {
                        text: error,
                        restart: None,
                    },
                );
                return EditorPresentation::empty(&plugin.identity.id, instance, region);
            }
        };
        if let Some(error) = runtime.error.clone() {
            ui.item(
                ("plugin-error", instance.0, region),
                rect,
                HostItem::Error {
                    text: error,
                    restart: Some(plugin.identity.id.clone()),
                },
            );
            return EditorPresentation::empty(&plugin.identity.id, instance, region);
        }
        let pass = runtime.pass;
        let target = Target { instance, region };
        let clip = ui.clip();
        ui.register(target, rect);
        let cropped = Quad::upright(rect).crop_to(clip);
        let visible = cropped
            .as_ref()
            .map_or(Rect::ZERO, |(_, source)| scale_rect(*source, rect.size()));
        let screen = runtime.instances.report(
            instance,
            region,
            &client,
            client_id,
            role,
            block_types,
            frame,
            rect.size(),
            visible,
            host::pixels_per_point(),
            pass,
        );
        if let Some(view) = view {
            runtime.instances.set_view(instance, view);
        }
        let drawn = runtime
            .layout
            .placement(screen)
            .map(|placement| (placement.width, placement.height));
        let held = runtime.instances.held(
            instance,
            region,
            cropped.as_ref().map(|(quad, _)| quad.rect),
            clip,
            drawn,
        );
        let (children, holes) = runtime
            .instances
            .host_children(instance, region, rect, clip);
        runtime.instances.place(
            instance,
            region,
            Placement {
                target,
                rect,
                clip,
                pass,
            },
        );
        let over_hole = host::pointer().is_some_and(|position| holes.contains(position));
        let drag = input::block_drag(rect.intersect(clip)).filter(|_| !over_hole);
        let hovering = drag.as_ref().is_some_and(|drag| !drag.dropped);
        let messages = runtime.instances.drag(instance, region, drag);
        runtime.send(messages);
        let files = input::file_drop(rect.intersect(clip)).filter(|_| !over_hole);
        let messages = runtime.instances.file_drop(instance, region, files);
        runtime.send(messages);
        if hovering && runtime.instances.drag_accepted(instance) {
            host::set_cursor(beui::CursorIcon::Alias);
        } else if host::hovered(target)
            && !over_hole
            && let Some(cursor) = runtime.instances.cursor(instance, region)
        {
            host::set_cursor(cursor);
        }
        if let Some(ime) = runtime.instances.ime(instance, region, rect) {
            host::set_ime(ime);
        }
        EditorPresentation {
            plugin_id: plugin.identity.id.clone(),
            instance,
            region,
            id: Some(target),
            loading_rect: plugin_loading_rect(&runtime.layout, screen, rect),
            screen: Some(screen),
            quad: match held {
                Some(held) => Some(Quad::upright(held.rect)),
                None => cropped.map(|(quad, _)| quad),
            },
            clip: match held {
                Some(held) => held.clip,
                None => clip,
            },
            drawn: held.map(|held| held.drawn),
            floating: match held {
                Some(_) => Vec::new(),
                None => runtime
                    .instances
                    .frame_report(instance)
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
            },
            open: runtime.instances.take_open(instance),
            drag: runtime.instances.take_block_drag(instance),
            command: runtime.instances.take_block_command(instance),
            children,
        }
    })
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
        client,
        client_id,
        instance,
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
            .report_creation(instance, &client, client_id, block_types)
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
        client,
        client_id,
        instance,
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
            &client,
            client_id,
            block_types,
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

pub(crate) fn preview(ui: &mut Ui, slot: PreviewSlot<'_>) -> PreviewPresentation {
    let PreviewSlot {
        plugin,
        block_types,
        client,
        client_id,
        block_id,
        block_type,
        instance,
        corners,
        opacity,
    } = slot;
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let Ok(runtime) = host.runtime(plugin) else {
            return PreviewPresentation::empty();
        };
        if runtime.error.is_some() {
            return PreviewPresentation::empty();
        }
        let rect = Rect::from_points(&corners);
        let scale_factor = host::pixels_per_point();
        let size = preview_size(rect.size(), scale_factor);
        let cropped = Quad {
            rect,
            corners,
            opacity,
        }
        .crop_to(ui.clip());
        let visible = cropped
            .as_ref()
            .map_or(Rect::ZERO, |(_, source)| scale_rect(*source, size));
        let pass = runtime.pass;
        let screen = runtime.instances.report(
            instance,
            EditorRegion::Preview,
            &client,
            client_id,
            InstanceRole::Editor(EditorBlock {
                id: block_id,
                block_type,
            }),
            block_types,
            None,
            size,
            visible,
            scale_factor,
            pass,
        );
        let (children, _) = runtime.instances.host_children(
            instance,
            EditorRegion::Preview,
            Rect::from_min_size(Pos2::ZERO, size),
            Rect::EVERYTHING,
        );
        let Some((quad, _)) = cropped else {
            return PreviewPresentation {
                drawn: false,
                size,
                children,
            };
        };
        let placed = runtime.layout.placement(screen).is_some();
        if placed {
            ui.blit(runtime.present(screen, quad, UNIT, None));
        }
        PreviewPresentation {
            drawn: placed,
            size,
            children,
        }
    })
}

impl PreviewPresentation {
    fn empty() -> Self {
        Self {
            drawn: false,
            size: Vec2::ZERO,
            children: Vec::new(),
        }
    }
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
    HOST.with(|host| {
        let mut host = host.borrow_mut();
        let overlay = std::mem::take(&mut host.overlay);
        for runtime in host.runtimes.values_mut() {
            runtime.detect_error();
            runtime.pump();
            runtime.begin_frame(pass, &overlay);
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

pub(crate) fn take_artifact_watch(
    plugin_id: &str,
    instance: EditorInstanceId,
) -> Option<Vec<Uuid>> {
    with(plugin_id, |runtime| {
        runtime.instances.take_artifact_watch(instance)
    })
    .flatten()
}

pub(crate) fn show_block(
    plugin_id: &str,
    instance: EditorInstanceId,
    block_id: Uuid,
    block_type: Uuid,
    via: Option<Uuid>,
    from: Option<Uuid>,
) {
    with(plugin_id, |runtime| {
        let messages = runtime
            .instances
            .show_block(instance, block_id, block_type, via, from);
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

pub(crate) fn cover_frame(plugin_id: &str, instance: EditorInstanceId, frame: Rect) {
    let Some(rects) = frame_rects(plugin_id, instance).map(|rects| {
        super::pieces::subtract(frame, &[rects.content.translate(frame.min.to_vec2())])
    }) else {
        return;
    };
    HOST.with(|host| {
        host.borrow_mut().overlay = FrameOverlay {
            owner: Some(instance),
            rects,
        };
    });
}

pub(crate) fn frame_rects(plugin_id: &str, instance: EditorInstanceId) -> Option<HostFrame> {
    with(plugin_id, |runtime| {
        runtime.instances.frame_report(instance).map(|report| {
            let rect = |rect: &block_plugin_api::ChildRect| {
                Rect::from_min_size(
                    pos2(rect.x, rect.y),
                    vec2(rect.width, rect.height),
                )
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

pub(crate) fn present(
    plugin_id: &str,
    instance: EditorInstanceId,
    presenting: bool,
) {
    with(plugin_id, |runtime| {
        if runtime.instances.set_presenting(instance, presenting) {
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
    Theme {
        dark: host::dark(),
    }
}

pub(super) fn with<R>(plugin_id: &str, act: impl FnOnce(&mut Runtime) -> R) -> Option<R> {
    HOST.with(|host| Some(act(host.borrow_mut().runtimes.get_mut(plugin_id)?)))
}

#[cfg(test)]
mod tests;
