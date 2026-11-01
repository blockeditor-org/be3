use beui::native::accessibility::Embedded;
use beui::{ImeArea, Pos2, Rect, Vec2, pos2, vec2};
use block_plugin_api::{
    AccessLevel, AnchorSpace, AnchoredPlacement, ArtifactDescription, AudioCommand, BlockCommand,
    BlockPick, ChildContent, ChildId, ChildMode, ChildPlacement, ChildPlacements, ChildStatus,
    DialogStatus, EditorInstanceId, FrameReport, FrameSpec, MenuEntry, Occluder, OpenArtifact,
    OpenBlock, OpenCreation, OpenPicker, OpenRename, Opening, PerformanceMeasurement, ScreenId,
    ScreenLayout, ScreenRequest, ScreenWindow, ShellCommand, Size, Stopped, View, ViewAnchor,
    ViewChange, ViewId, ViewPlacement, ViewSize, ViewportMetrics, WebViewEvent, WebViewId,
};
use block_plugin_api::{
    DescribedAction, HostView, WindowOpen, WindowRequests,
    embed::{debug::debug::Call as DebugCall, shell::Call as ShellCall},
};
use block_type_guest::editor::{FromEditor, Gesture};
use stable_hash::{HashMap, HashSet};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

use super::{
    ChildCommit, EditorBlock, HostAnchored, HostChild, HostChildStatus, InstanceRole,
    MAX_LIVE_CHILDREN,
    audio::AudioPlayer,
    calls::{self, BlockIds, EditorAnswer, EditorCall, Outbound, PluginCall},
    heads::{HeadReads, Heads},
    input::{BlockDrag, BlockDragEvent, FileDropEvent, InputForwarder, viewport_metrics},
    pieces,
    published::{Published, Watched},
    sent::Sent,
};
use block_app_base::{host, performance};

mod services;

thread_local! {
    static LAYERS_OPENED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static PRESENCE_WANTED: std::cell::RefCell<stable_hash::HashSet<EditorInstanceId>> =
        std::cell::RefCell::default();
}

pub(super) fn want_presence(instance: EditorInstanceId, visible: bool) {
    PRESENCE_WANTED.with_borrow_mut(|wanted| match visible {
        true => wanted.insert(instance),
        false => wanted.remove(&instance),
    });
}

fn presence_wanted(instance: EditorInstanceId) -> bool {
    PRESENCE_WANTED.with_borrow(|wanted| wanted.contains(&instance))
}

#[derive(Default)]
pub(super) struct Instances {
    entries: HashMap<EditorInstanceId, Instance>,
    focus: Focus,
    connection: Option<Connection>,
    next_screen: u64,
    announced: HashSet<ScreenId>,
    unannounced_input: Vec<Outbound>,
    block_types: Option<Arc<crate::PluginCatalog>>,
    sent_block_types: Option<(u64, u64)>,
    network: Vec<String>,
    plugin_id: String,
    services: services::Services,
    epoch: u64,
    graph_seen: Option<u64>,
    audio_changes: AudioChanges,
    head_reads: HeadReads,
    published: Published,
    shell: Option<EditorInstanceId>,
    waiting: HashMap<EditorInstanceId, Vec<Outbound>>,
    gone: HashSet<EditorInstanceId>,
    serial: u64,
    owners: HashMap<block_plugin_api::ops::OpId, EditorInstanceId>,
    early_shows: HashMap<EditorInstanceId, Vec<block_plugin_api::blocks::ShowRequest>>,
    accepted: Vec<block_plugin_api::ops::OpId>,
    relaying: HashSet<EditorInstanceId>,
}

struct AudioChanges {
    sender: host::WakingSender<EditorInstanceId>,
    receiver: std::sync::mpsc::Receiver<EditorInstanceId>,
}

impl Default for AudioChanges {
    fn default() -> Self {
        let (sender, receiver) = host::waking_channel();
        Self { sender, receiver }
    }
}

struct Connection {
    client_id: Uuid,
}

struct Instance {
    role: InstanceRole,
    artifact: ArtifactState,
    dialog: DialogStatus,
    child_dismissals: Vec<ChildId>,
    created: Option<Result<Uuid, String>>,
    screens: HashMap<ViewId, Screen>,
    anchored: Vec<AnchoredScreen>,
    opened: bool,
    deferred: Vec<Outbound>,
    opens: Vec<OpenRequest>,
    block_drags: Vec<BlockDrag>,
    block_commands: Vec<(Uuid, BlockCommand)>,
    shell_commands: Vec<(Uuid, ShellCommand)>,
    simulated: Vec<(Uuid, AccessLevel)>,
    reported_focus: Option<Focus>,
    reported_editable: Option<bool>,
    focus_reports: Vec<Focus>,
    artifact_watch: Option<Vec<Uuid>>,
    heads: Heads,
    revision_previews: HashSet<Uuid>,
    drag_accepted: bool,
    intrinsic: Option<Vec2>,
    aspect_ratio: Option<f32>,
    audio: Option<AudioPlayer>,
    reported_size: Option<Vec2>,
    picked: Option<BlockPick>,
    card: bool,
    child_commits: Vec<ChildCommit>,
    view: Option<EditorView>,
    reported_view: Option<EditorView>,
    views: Sent<EditorView>,
    view_changes: Vec<ViewChange>,
    bar_actions: Vec<block_plugin_api::BarAction>,
    menus: BTreeMap<ViewId, Vec<block_plugin_api::MenuEntry>>,
    child_menu_picks: Vec<(ChildId, String, Option<Rect>)>,
    presenting: bool,
    presented: bool,
    reported_presenting: bool,
    published: Watched,
    unshown_web_views: Vec<WebViewId>,
    presence_visible: Option<bool>,
    replacements: HashMap<(Uuid, Uuid), Replacement>,
    next_replacement: u64,
    leaving: bool,
    content: Option<ContentLink>,
    watched: HashMap<Uuid, ContentLink>,
    shown: stable_hash::HashSet<(Uuid, Uuid)>,
    replicas: HashMap<Uuid, block_plugin_api::ops::OpId>,
    watches: HashMap<block_plugin_api::ops::OpId, blocks::Watch>,
    shows: Vec<block_plugin_api::blocks::ShowRequest>,
    blocks_seen: Option<u64>,
    version_sent: Option<u64>,
    stale: bool,
}

struct ContentLink {
    content_type: Uuid,
    attached: Option<bool>,
    delivered: bool,
    awaiting_preview: bool,
    gesturing: bool,
    described: Option<be_block::Entries>,
}

impl ContentLink {
    fn new(content_type: Uuid) -> Self {
        Self {
            content_type,
            attached: None,
            delivered: false,
            awaiting_preview: false,
            gesturing: false,
            described: None,
        }
    }

    fn describe(&mut self, block: Uuid) {
        if !block_app_stack::access(block).can_edit() {
            return;
        }
        let Some(derived) = block_app_stack::describe_block(block) else {
            return;
        };
        if self.described.as_ref() == Some(&derived) {
            return;
        }
        self.described = Some(derived.clone());
        block_app_stack::describe_implicitly(block, derived);
    }

    fn detach(&mut self, instance: EditorInstanceId, block: Uuid, shown: &[Uuid]) {
        if self.attached.take().is_none() {
            return;
        }
        let mut goodbyes = Vec::new();
        if self.gesturing {
            goodbyes.push(FromEditor::Gesture(Gesture::Cancel));
        }
        for kind in shown {
            goodbyes.push(FromEditor::ShowPresence {
                kind: *kind,
                value: None,
            });
        }
        for goodbye in goodbyes {
            block_app_stack::from_editor(
                block,
                instance.0,
                &block_type_api::encode_frame(&goodbye),
            );
        }
        block_app_stack::detach(block, instance.0);
    }
}

pub(super) type OpenRequest = crate::BlockOpen;

#[derive(Clone, Default, PartialEq)]
pub struct Focus {
    pub block: Option<(Uuid, Uuid)>,
    pub via: Vec<Uuid>,
}

#[derive(Clone, Copy)]
pub(super) struct Held {
    pub(super) rect: Rect,
    pub(super) drawn: (u32, u32),
}

#[derive(Clone, Copy, PartialEq)]
pub struct EditorView {
    pub rect: Rect,
    pub scale: f32,
}

enum Replacement {
    Pending(u64),
    Answered(bool),
}

#[derive(Default)]
struct ArtifactState {
    data: Vec<u8>,
    description: Option<ArtifactDescription>,
    draft: Option<Vec<u8>>,
    outcome: Option<Result<(), String>>,
}

mod blocks;
use blocks::Subject;

impl Instance {
    fn new(role: InstanceRole) -> Self {
        Self {
            role,
            artifact: ArtifactState::default(),
            dialog: DialogStatus::default(),
            child_dismissals: Vec::new(),
            created: None,
            screens: HashMap::default(),
            anchored: Vec::new(),
            opened: false,
            deferred: Vec::new(),
            opens: Vec::new(),
            block_drags: Vec::new(),
            block_commands: Vec::new(),
            shell_commands: Vec::new(),
            simulated: Vec::new(),
            reported_focus: None,
            reported_editable: None,
            focus_reports: Vec::new(),
            artifact_watch: None,
            heads: Heads::default(),
            revision_previews: HashSet::default(),
            drag_accepted: false,
            intrinsic: None,
            aspect_ratio: None,
            audio: None,
            reported_size: None,
            picked: None,
            card: false,
            child_commits: Vec::new(),
            view: None,
            reported_view: None,
            views: Sent::default(),
            view_changes: Vec::new(),
            bar_actions: Vec::new(),
            menus: BTreeMap::new(),
            child_menu_picks: Vec::new(),
            presenting: false,
            presented: false,
            reported_presenting: false,
            published: Watched::default(),
            unshown_web_views: Vec::new(),
            presence_visible: None,
            replacements: HashMap::default(),
            next_replacement: 0,
            leaving: false,
            watched: HashMap::default(),
            shown: stable_hash::HashSet::default(),
            replicas: HashMap::default(),
            watches: HashMap::default(),
            shows: Vec::new(),
            blocks_seen: None,
            version_sent: None,
            stale: true,
            content: match role {
                InstanceRole::Editor(_, block) => own_content_type(block).map(ContentLink::new),
                InstanceRole::Creation(..)
                | InstanceRole::Artifact(..)
                | InstanceRole::Picker(..)
                | InstanceRole::Rename(..) => None,
            },
        }
    }
}

fn own_content_type(block: EditorBlock) -> Option<Uuid> {
    block_app_stack::provided(block.block_type).then_some(block.block_type)
}

impl Instance {
    fn offer_blocks(&mut self, scope: Option<block_app_stack::Scope>) {
        let revision = block_app_stack::graph_revision();
        if self.blocks_seen == Some(revision) {
            return;
        }
        self.blocks_seen = Some(revision);
        let queries: Vec<_> = self
            .watching()
            .filter_map(|watched| match watched {
                Subject::Blocks(query) => Some(query),
                _ => None,
            })
            .collect();
        for query in queries {
            let graph_query = block_app_stack::plugin_graph::query_of(query);
            let mut answer = match block_app_stack::ready(graph_query) {
                true => Ok(block_app_stack::plugin_graph::answer(query)),
                false => match block_app_stack::failure(graph_query) {
                    Some(error) => Err(error),
                    None => continue,
                },
            };
            if let Ok(blocks) = &mut answer {
                Instances::translated(scope, false, |visit| {
                    for block in blocks.iter_mut() {
                        block.visit_block_ids(visit);
                    }
                });
            }
            self.offer(Subject::Blocks(query), &answer);
        }
    }

    fn wants(&self) -> Vec<block_app_stack::Query> {
        let mut wants: Vec<block_app_stack::Query> = self
            .watching()
            .filter_map(|watched| match watched {
                Subject::Blocks(query) => Some(block_app_stack::plugin_graph::query_of(query)),
                _ => None,
            })
            .collect();
        if let Some(block) = self.role.block() {
            wants.push(block_app_stack::Query::Parents(block.id));
        }
        wants.extend(
            self.watched
                .keys()
                .map(|block| block_app_stack::Query::Block(*block)),
        );
        wants
    }

    fn offer_version(&mut self) {
        if !self.watching().any(|watched| watched == Subject::Version) {
            return;
        }
        if let InstanceRole::Editor(_, block) = self.role
            && block_app_stack::is_versioned(block.block_type)
            && let Some((revision, status)) =
                block_app_stack::version_since(block.id, block.block_type, self.version_sent)
        {
            self.version_sent = Some(revision);
            self.offer(Subject::Version, &status);
        }
    }

    fn link_mut(&mut self, block: Uuid) -> Option<&mut ContentLink> {
        if self.role.block().is_some_and(|own| own.id == block) {
            return self.content.as_mut();
        }
        self.watched.get_mut(&block)
    }

    fn holds(&self, block: Uuid) -> bool {
        (self.content.is_some() && self.role.block().is_some_and(|own| own.id == block))
            || self.watched.contains_key(&block)
    }

    fn shown_on(&self, block: Uuid) -> Vec<Uuid> {
        self.shown
            .iter()
            .filter(|(shown, _)| *shown == block)
            .map(|(_, kind)| *kind)
            .collect()
    }

    fn reattach(&mut self, instance: EditorInstanceId) {
        let own = self.role.block().map(|block| block.id);
        for (block, link) in self
            .content
            .iter_mut()
            .filter_map(|link| own.map(|own| (own, link)))
            .chain(self.watched.iter_mut().map(|(block, link)| (*block, link)))
        {
            let Some(attached) = link.attached else {
                continue;
            };
            let editable = block_app_stack::access(block).can_edit();
            if attached != editable {
                link.attached = Some(editable);
                block_app_stack::attach(block, link.content_type, instance.0, editable);
            }
        }
    }

    fn relay_frames(
        &mut self,
        instance: EditorInstanceId,
        send: &mut impl FnMut(block_plugin_api::ops::OpId, &Vec<u8>) -> bool,
    ) {
        let mut blocked = HashSet::default();
        let mut kept = Vec::new();
        for (block, frame) in block_app_stack::take_editor_frames(instance.0) {
            if blocked.contains(&block) {
                kept.push((block, frame));
                continue;
            }
            match self.relay_frame(block, &frame, send) {
                true => {}
                false => {
                    blocked.insert(block);
                    kept.push((block, frame));
                }
            }
        }
        block_app_stack::keep_editor_frames(instance.0, kept);
        let own = self.role.block().map(|block| block.id);
        let previews: Vec<Uuid> = self
            .content
            .iter()
            .filter_map(|link| own.map(|own| (own, link)))
            .chain(self.watched.iter().map(|(block, link)| (*block, link)))
            .filter(|(block, link)| link.awaiting_preview && !blocked.contains(block))
            .map(|(block, _)| block)
            .collect();
        for block in previews {
            let Some(frame) = block_app_stack::snapshot_frame(block) else {
                continue;
            };
            if self.relay_frame(block, &frame, send)
                && let Some(link) = self.link_mut(block)
            {
                link.awaiting_preview = false;
            }
        }
    }

    fn relay_frame(
        &mut self,
        block: Uuid,
        frame: &Vec<u8>,
        send: &mut impl FnMut(block_plugin_api::ops::OpId, &Vec<u8>) -> bool,
    ) -> bool {
        let Some(replica) = self.replicas.get(&block).copied() else {
            return true;
        };
        if self.link_mut(block).is_none() {
            return true;
        }
        if !send(replica, frame) {
            return false;
        }
        if let Some(link) = self.link_mut(block) {
            link.delivered = true;
        }
        true
    }

    fn follows(&self, block: Uuid) -> bool {
        self.role.block().is_some_and(|own| {
            own.id == block || block_app_stack::revisions::source(own.id) == Some(block)
        }) || self.watched.contains_key(&block)
            || self
                .watching()
                .any(|watched| watched == Subject::History(block))
            || self.heads.follows(block)
    }
}

impl Instance {
    fn offer_histories(&mut self) {
        let blocks: Vec<Uuid> = self
            .watching()
            .filter_map(|watched| match watched {
                Subject::History(block) => Some(block),
                _ => None,
            })
            .collect();
        for block in blocks {
            let history = self.history(block);
            self.offer(Subject::History(block), &history);
        }
    }

    fn name_content(&mut self) {
        if self
            .role
            .block()
            .is_some_and(|block| block_app_stack::revisions::source(block.id).is_some())
        {
            return;
        }
        if let (Some(block), Some(link)) = (self.role.block(), self.content.as_mut()) {
            link.describe(block.id);
        }
        for (block, link) in &mut self.watched {
            link.describe(*block);
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Placement {
    pub(super) rect: Rect,
    pub(super) clip: Rect,
}

pub(super) struct Described<'a> {
    pub(super) screen: ScreenId,
    pub(super) actions: &'a [DescribedAction],
    pub(super) focused: bool,
    pub(super) tree: &'a mut Embedded,
}

struct Screen {
    input: InputForwarder,
    placement: Option<Placement>,
    request: ScreenRequest,
    last_seen: u64,
    mounted: u32,
    presented: Option<(Rect, Rect)>,
    holding: bool,
    used: Option<Vec2>,
    report: Option<FrameReport>,
    accessibility: Embedded,
    focus_requested: bool,
    dragging: bool,
    file_dropping: bool,
    window: WindowRequests,
    stopped: Option<Stopped>,
    children: ChildTable,
    reported_statuses: HashMap<ChildId, ChildStatus>,
    revoked: HashSet<ChildId>,
    frame_revoked: HashSet<ChildId>,
    scales: Sent<f32>,
    spaces: Sent<Option<(AnchorSpace, f32)>>,
    fresh_space: Option<AnchorSpace>,
    serial: u64,
    kept: Option<ViewportMetrics>,
}

impl Screen {
    fn new(request: ScreenRequest, pass: u64) -> Self {
        Self {
            input: InputForwarder::default(),
            placement: None,
            request,
            last_seen: pass,
            mounted: 0,
            presented: None,
            holding: false,
            used: None,
            report: None,
            accessibility: Embedded::new(beui::native::accessibility::next_document_id()),
            focus_requested: false,
            dragging: false,
            file_dropping: false,
            window: WindowRequests::default(),
            stopped: None,
            children: ChildTable::default(),
            reported_statuses: HashMap::default(),
            revoked: HashSet::default(),
            frame_revoked: HashSet::default(),
            scales: Sent::default(),
            spaces: Sent::default(),
            fresh_space: None,
            serial: 0,
            kept: None,
        }
    }

    fn shown_scale(&self, rect: Rect) -> Vec2 {
        let metrics = &self.request.metrics;
        let painted = self
            .scales
            .at(self.serial)
            .copied()
            .unwrap_or(metrics.scale_factor);
        vec2(
            ratio(rect.width(), metrics.logical.width),
            ratio(rect.height(), metrics.logical.height),
        ) * ratio(painted, metrics.scale_factor)
    }

    fn anchor_shift(&self) -> Option<(AnchorSpace, Vec2, f32)> {
        self.request.anchor_space?;
        let now = self.fresh_space.or(self.request.anchor_space)?;
        let (laid, raster) = self
            .spaces
            .at(self.serial)
            .copied()
            .flatten()
            .unwrap_or((now, self.request.metrics.raster));
        let shift = now.to_host.origin - laid.to_host.origin;
        let scale = laid.to_host.scale.x;
        let pixels_per_point = match scale.is_finite() && scale > 0.0 {
            true => raster / scale,
            false => raster,
        };
        Some((now, shift, pixels_per_point))
    }

    fn unplace(&mut self) {
        self.placement = None;
        self.request.metrics = viewport_metrics(Vec2::ZERO, Rect::ZERO, 1.0, 1.0);
    }
}

struct AnchoredScreen {
    view: ViewId,
    root: ViewId,
    opened: u64,
    placement: AnchoredPlacement,
    surface: u32,
    screen: Screen,
}

fn host_anchored(root: &Screen, anchored: &[AnchoredScreen], region: ViewId) -> Vec<HostAnchored> {
    let (Some(_), Some((space, shift, pixels_per_point))) = (root.placement, root.anchor_shift())
    else {
        return Vec::new();
    };
    let bounds = space.bounds;
    anchored
        .iter()
        .filter(|anchored| anchored.root == region)
        .filter_map(|anchored| {
            let report = anchored.screen.report.as_ref()?;
            let placement = &anchored.placement;
            let shift = match placement.anchor {
                ViewAnchor::Edge(_) => Vec2::ZERO,
                ViewAnchor::Rect(_) => shift,
            };
            let to_host = placement.rect.min.to_vec2() + shift;
            let within = |rects: &[Rect]| -> Vec<Rect> {
                rects
                    .iter()
                    .map(|rect| rect.translate(to_host).intersect(bounds))
                    .filter(|rect| rect.is_positive())
                    .collect()
            };
            Some(HostAnchored {
                id: anchored.view,
                parent: (placement.parent != region).then_some(placement.parent),
                opened: anchored.opened,
                screen: anchored.screen.request.screen,
                rect: placement.rect.translate(shift),
                bounds,
                painted: within(&report.painted),
                floating: within(&report.floating),
                scrims: report
                    .scrims
                    .iter()
                    .filter_map(|scrim| {
                        let rect = scrim.rect.translate(to_host).intersect(bounds);
                        rect.is_positive().then(|| beui::layer::Scrim {
                            rect,
                            color: crate::runtime::scrim_color(&scrim.color),
                            radius: scrim.radius,
                        })
                    })
                    .collect(),
                surface: anchored.surface,
                pixels_per_point,
                dismiss_on_outside_press: placement.dismiss_on_outside_press,
                trigger: placement.trigger.map(|rect| rect.translate(shift)),
                keyboard: placement.keyboard,
                blocks_owner: placement.blocks_owner,
                edge: matches!(placement.anchor, ViewAnchor::Edge(_)),
            })
        })
        .collect()
}

#[derive(Default, PartialEq)]
struct ChildTable {
    children: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
}

struct Hole {
    rect: Rect,
    occluders: Vec<Rect>,
}

#[derive(Default)]
pub(super) struct Holes {
    holes: Vec<Hole>,
}

impl Holes {
    pub(super) fn parts(&self) -> Vec<(Rect, Vec<Rect>)> {
        self.holes
            .iter()
            .map(|hole| (hole.rect, hole.occluders.clone()))
            .collect()
    }
}

pub(super) struct NextScreens {
    pub(super) opened: Vec<Outbound>,
    pub(super) screens: Vec<ScreenRequest>,
}

impl Instances {
    pub(super) fn translate(&self, message: &mut impl BlockIds, inward: bool) {
        let mut carries = false;
        message.visit_block_ids(&mut |_, _, _| carries = true);
        if !carries {
            return;
        }
        let mut scopes: HashMap<EditorInstanceId, Option<block_app_stack::Scope>> =
            HashMap::default();
        message.visit_block_ids(&mut |instance, _, _| {
            scopes.entry(instance).or_insert_with(|| {
                let block = self.entries.get(&instance)?.role.block()?.id;
                let source = block_app_stack::revisions::source(block).unwrap_or(block);
                block_app_stack::with_graph(|graph| graph.scope_of(source)).flatten()
            });
        });
        block_app_stack::with_graph(|graph| {
            message.visit_block_ids(&mut |instance, role, id| {
                let scope = scopes.get(&instance).copied().flatten();
                let Some(scope) = scope else {
                    return;
                };
                let block = Uuid::from_bytes(*id);
                let translated = match inward {
                    true => graph.to_real(scope, block, role),
                    false => graph.to_local(scope, block),
                };
                *id = translated.into_bytes();
            });
        });
    }

    pub(super) fn hold_unreported(&mut self, messages: Vec<Outbound>) -> Vec<Outbound> {
        let mut passed = Vec::with_capacity(messages.len());
        for message in messages {
            match message.instance() {
                Some(instance) if !message.closes() && !self.entries.contains_key(&instance) => {
                    if !self.gone.contains(&instance) {
                        self.waiting.entry(instance).or_default().push(message);
                    }
                }
                _ => passed.push(message),
            }
        }
        passed
    }

    pub(super) fn take_reported(&mut self) -> Vec<Outbound> {
        if self.waiting.is_empty() {
            return Vec::new();
        }
        let reported: Vec<EditorInstanceId> = self
            .waiting
            .keys()
            .copied()
            .filter(|instance| self.entries.contains_key(instance))
            .collect();
        reported
            .into_iter()
            .flat_map(|instance| self.waiting.remove(&instance).unwrap_or_default())
            .collect()
    }

    pub(super) fn gate(&mut self, messages: Vec<Outbound>) -> Vec<Outbound> {
        let mut allowed = Vec::with_capacity(messages.len());
        for message in messages {
            if let Some(instance) = message.instance() {
                match self.entries.get_mut(&instance) {
                    Some(entry) if entry.opened => allowed.push(message),
                    Some(entry) => entry.deferred.push(message),
                    None if message.closes() => allowed.push(message),
                    None => {}
                }
                continue;
            }
            match &message {
                Outbound::Plugin(PluginCall::Input(batch))
                    if !self.announced.contains(&batch.screen)
                        && !self.is_anchored_screen(batch.screen) =>
                {
                    if self.is_screen(batch.screen) {
                        self.unannounced_input.push(message);
                    }
                }
                _ => allowed.push(message),
            }
        }
        allowed
    }

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn remove(&mut self, instance: EditorInstanceId) -> bool {
        self.withdraw_all(instance);
        self.waiting.remove(&instance);
        self.gone.insert(instance);
        self.early_shows.remove(&instance);
        let Some(entry) = self.entries.remove(&instance) else {
            return false;
        };
        self.owners.retain(|_, owner| *owner != instance);
        block_app_stack::want(block_app_stack::Owner::Editor(instance.0), Vec::new());
        let mut entry = entry;
        let shown: Vec<(Uuid, Uuid)> = entry.shown.iter().copied().collect();
        let shown_on = |block: Uuid| -> Vec<Uuid> {
            shown
                .iter()
                .filter(|(on, _)| *on == block)
                .map(|(_, kind)| *kind)
                .collect()
        };
        if let Some(block) = entry.role.block()
            && let Some(link) = entry.content.as_mut()
        {
            link.detach(instance, block.id, &shown_on(block.id));
        }
        for (block, link) in &mut entry.watched {
            link.detach(instance, *block, &shown_on(*block));
        }
        for preview in &entry.revision_previews {
            block_app_stack::revisions::release(*preview);
        }
        entry.opened
    }

    fn editable(&self, block: Uuid) -> bool {
        block_app_stack::access(block).can_edit()
    }

    fn replica_frame(&mut self, instance: EditorInstanceId, block: Uuid, frame: Vec<u8>) -> bool {
        let editable = self.editable(block);
        let viewable = block_app_stack::may_load(block);
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        let message = block_app_stack::unframe(&frame).and_then(|bytes| FromEditor::decode(&bytes));
        if let Some(FromEditor::ShowPresence { kind, value }) = &message {
            match value.is_some() {
                true => entry.shown.insert((block, *kind)),
                false => entry.shown.remove(&(block, *kind)),
            };
        }
        let Some(link) = entry.link_mut(block) else {
            return false;
        };
        match &message {
            Some(FromEditor::Gesture(Gesture::Begin)) => link.gesturing = true,
            Some(FromEditor::Gesture(Gesture::Commit | Gesture::Cancel)) => link.gesturing = false,
            _ => {}
        }
        let resend = matches!(message, Some(FromEditor::Resend));
        if let Some(source) = block_app_stack::revisions::source(block) {
            block_app_stack::revisions::load(block, source, link.content_type);
            if resend {
                link.awaiting_preview = true;
            }
            return resend;
        }
        if !viewable {
            return false;
        }
        let attaching = link.attached.is_none();
        if link.attached != Some(editable) {
            link.attached = Some(editable);
            block_app_stack::attach(block, link.content_type, instance.0, editable);
        }
        if resend && block_app_stack::snapshot_frame(block).is_some() {
            link.awaiting_preview = true;
        }
        if !(resend && attaching) {
            block_app_stack::from_editor(block, instance.0, &frame);
        }
        true
    }

    fn can_view(&self, block: Uuid) -> bool {
        block_app_stack::access(block).can_view()
    }

    fn connect(&mut self, client_id: Uuid) {
        self.connection = Some(Connection { client_id });
    }

    pub(super) fn report(
        &mut self,
        instance: EditorInstanceId,
        shown: View,
        client_id: Uuid,
        role: InstanceRole,
        block_types: &Arc<crate::PluginCatalog>,
        frame: Option<FrameSpec>,
        size: Vec2,
        visible: Rect,
        scale_factor: f32,
        pass: u64,
    ) -> ScreenId {
        self.connect(client_id);
        if self.block_types.is_none() {
            self.block_types = Some(Arc::clone(block_types));
        }
        let entry = self
            .entries
            .entry(instance)
            .or_insert_with(|| Instance::new(role));
        if let Some(shows) = self.early_shows.remove(&instance) {
            for show in shows {
                block_plugin_api::blocks::ShowRequest::queue(&mut entry.shows, show);
            }
        }
        let next_screen = &mut self.next_screen;
        let screen = entry.screens.entry(shown.id).or_insert_with(|| {
            *next_screen += 1;
            Screen::new(
                ScreenRequest {
                    screen: ScreenId(*next_screen),
                    instance,
                    view: shown.id,
                    placement: shown.placement,
                    metrics: viewport_metrics(size, visible, scale_factor, scale_factor),
                    frame,
                    windows: false,
                    host_place: None,
                    origin: None,
                    docked: false,
                    anchor_space: None,
                },
                pass,
            )
        });
        screen.request.metrics = viewport_metrics(size, visible, scale_factor, scale_factor);
        screen.request.frame = frame;
        screen.last_seen = pass;
        screen.request.screen
    }

    pub(super) fn mount(
        &mut self,
        instance: EditorInstanceId,
        shown: View,
        client_id: Uuid,
        role: InstanceRole,
        block_types: &Arc<crate::PluginCatalog>,
    ) {
        self.report(
            instance,
            shown,
            client_id,
            role,
            block_types,
            None,
            Vec2::ZERO,
            Rect::ZERO,
            1.0,
            0,
        );
        if let Some(screen) = self.screen_mut(instance, shown.id) {
            screen.mounted += 1;
        }
    }

    pub(super) fn unmount(&mut self, instance: EditorInstanceId, region: ViewId) {
        if let Some(screen) = self.screen_mut(instance, region) {
            screen.mounted = screen.mounted.saturating_sub(1);
            if screen.mounted == 0 {
                screen.unplace();
            }
        }
    }

    pub(super) fn unplace(&mut self, instance: EditorInstanceId, region: ViewId) {
        if let Some(screen) = self.screen_mut(instance, region) {
            screen.unplace();
        }
    }

    pub(super) fn awaiting_content(&self) -> bool {
        self.entries.values().any(|entry| {
            let mounted = entry.screens.values().any(|screen| screen.mounted > 0);
            match (entry.role.block(), &entry.content) {
                (Some(block), Some(link)) => {
                    mounted
                        && link.attached.is_some()
                        && !link.delivered
                        && block_app_stack::may_load(block.id)
                }
                _ => false,
            }
        })
    }

    pub(super) fn has_mounted(&self) -> bool {
        self.entries.iter().any(|(instance, entry)| {
            entry.screens.values().any(|screen| screen.mounted > 0)
                || entry
                    .screens
                    .get(&ViewId::MAIN)
                    .is_some_and(|screen| screen.kept.is_some() && super::docks::docked(*instance))
        })
    }

    pub(super) fn on_screen(&self, instance: EditorInstanceId) -> bool {
        self.entries
            .get(&instance)
            .is_some_and(|entry| entry.screens.values().any(|screen| screen.mounted > 0))
    }

    pub(super) fn docked_children(
        &self,
    ) -> Vec<(EditorInstanceId, Vec<super::docks::DockedChild>)> {
        self.entries
            .iter()
            .map(|(instance, entry)| {
                let revision = entry
                    .role
                    .block()
                    .is_some_and(|block| block_app_stack::revisions::source(block.id).is_some());
                let children = entry
                    .screens
                    .values()
                    .filter(|screen| screen.mounted > 0 && !revision)
                    .flat_map(|screen| &screen.children.children)
                    .filter_map(|child| match (&child.content, child.docks) {
                        (ChildContent::Block { block_id, .. }, Some(tab)) => {
                            Some(super::docks::DockedChild {
                                child: child.child,
                                tab,
                                block: Uuid::from_bytes(*block_id),
                            })
                        }
                        _ => None,
                    })
                    .collect();
                (*instance, children)
            })
            .collect()
    }

    pub(super) fn place_mounted(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        frame: Option<FrameSpec>,
        size: Vec2,
        visible: Rect,
        scale_factor: f32,
        raster: f32,
        windows: bool,
        host_place: Option<String>,
        origin: Option<Pos2>,
        anchor_space: Option<AnchorSpace>,
    ) {
        let Some(screen) = self.screen_mut(instance, region) else {
            return;
        };
        screen.fresh_space = anchor_space;
        if !only_moved(screen.request.anchor_space, anchor_space) {
            screen.request.anchor_space = anchor_space;
        }
        let metrics = viewport_metrics(size, visible, scale_factor, raster);
        if region == ViewId::MAIN && metrics.pixel_width > 0 && metrics.pixel_height > 0 {
            screen.kept = Some(metrics);
        }
        if screen.request.metrics != metrics
            || screen.request.frame != frame
            || screen.request.windows != windows
            || screen.request.host_place != host_place
            || screen.request.origin != origin
        {
            screen.request.metrics = metrics;
            screen.request.frame = frame;
            screen.request.windows = windows;
            screen.request.host_place = host_place;
            screen.request.origin = origin;
        }
    }

    pub(super) fn refresh_anchoring_views(&mut self) -> bool {
        let hosting: Vec<(EditorInstanceId, ViewId)> = self
            .entries
            .iter()
            .flat_map(|(instance, entry)| {
                entry
                    .anchored
                    .iter()
                    .map(move |anchored| (*instance, anchored.root))
            })
            .collect();
        hosting
            .into_iter()
            .fold(false, |refreshed, (instance, region)| {
                self.refresh_anchor_space(instance, region) || refreshed
            })
    }

    pub(super) fn refresh_anchor_space(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
    ) -> bool {
        let Some(screen) = self.screen_mut(instance, region) else {
            return false;
        };
        let fresh = screen.fresh_space;
        let changed = screen.request.anchor_space != fresh;
        screen.request.anchor_space = fresh;
        changed
    }

    pub(super) fn stopped(&self, instance: EditorInstanceId, region: ViewId) -> Option<&Stopped> {
        self.entries
            .get(&instance)?
            .screens
            .get(&region)?
            .stopped
            .as_ref()
    }

    pub(super) fn placement(
        &self,
        instance: EditorInstanceId,
        region: ViewId,
    ) -> Option<Placement> {
        self.entries.get(&instance)?.screens.get(&region)?.placement
    }

    pub(super) fn request_of(&self, screen: ScreenId) -> Option<&ScreenRequest> {
        self.entries.values().find_map(|entry| {
            entry
                .screens
                .values()
                .map(|held| &held.request)
                .chain(entry.anchored.iter().map(|held| &held.screen.request))
                .find(|request| request.screen == screen)
        })
    }

    pub(super) fn screen_id(&self, instance: EditorInstanceId, region: ViewId) -> Option<ScreenId> {
        Some(
            self.entries
                .get(&instance)?
                .screens
                .get(&region)?
                .request
                .screen,
        )
    }

    pub(super) fn back(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        gesture: beui::BackGesture,
    ) -> Vec<PluginCall> {
        let announced = &self.announced;
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
            .filter(|screen| announced.contains(&screen.request.screen))
        else {
            return Vec::new();
        };
        vec![PluginCall::Input(block_plugin_api::InputBatch {
            screen: screen.request.screen,
            every_sample: false,
            events: vec![block_plugin_api::Input::Back(gesture).into()],
            dismissed: Vec::new(),
        })]
    }

    pub(super) fn intercepted(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        press: Option<beui::KeyPress>,
        modifiers: beui::Modifiers,
    ) -> Vec<PluginCall> {
        let announced = &self.announced;
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
            .filter(|screen| announced.contains(&screen.request.screen))
        else {
            return Vec::new();
        };
        let events = screen.input.intercepted(press, modifiers);
        if events.is_empty() {
            return Vec::new();
        }
        vec![PluginCall::Input(block_plugin_api::InputBatch {
            screen: screen.request.screen,
            every_sample: false,
            events,
            dismissed: Vec::new(),
        })]
    }

    pub(super) fn intercepted_tap(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        key: beui::Key,
    ) -> Vec<PluginCall> {
        let announced = &self.announced;
        let Some(screen) = self
            .entries
            .get(&instance)
            .and_then(|entry| entry.screens.get(&region))
            .filter(|screen| announced.contains(&screen.request.screen))
            .filter(|screen| !screen.input.focused())
        else {
            return Vec::new();
        };
        if key == beui::Key::Unidentified {
            return Vec::new();
        }
        vec![PluginCall::Input(block_plugin_api::InputBatch {
            screen: screen.request.screen,
            every_sample: false,
            events: vec![block_plugin_api::Input::InterceptedTap(key).into()],
            dismissed: Vec::new(),
        })]
    }

    pub(super) fn mounted(&self, instance: EditorInstanceId, region: ViewId) -> bool {
        self.entries
            .get(&instance)
            .and_then(|entry| entry.screens.get(&region))
            .is_some_and(|screen| screen.mounted > 0)
    }

    pub(super) fn view_placement(
        &self,
        instance: EditorInstanceId,
        view: ViewId,
    ) -> Option<ViewPlacement> {
        Some(
            self.entries
                .get(&instance)?
                .screens
                .get(&view)?
                .request
                .placement,
        )
    }

    fn screen_mut(&mut self, instance: EditorInstanceId, region: ViewId) -> Option<&mut Screen> {
        self.entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
    }

    pub(super) fn forward(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        input: &beui::ForwardedInput,
        aim: &dyn Fn(Option<beui::Over>, beui::Pos2) -> beui::Aim,
        target: &dyn Fn(beui::NodeId) -> Option<(EditorInstanceId, ViewId)>,
    ) -> (Vec<Outbound>, bool) {
        let screens: Vec<(EditorInstanceId, ViewId, ScreenId)> = self
            .entries
            .iter()
            .flat_map(|(instance, entry)| {
                entry
                    .screens
                    .iter()
                    .filter(|(_, screen)| self.announced.contains(&screen.request.screen))
                    .map(|(region, screen)| (*instance, *region, screen.request.screen))
            })
            .collect();
        let screen_of = |node: beui::NodeId| {
            let (instance, region) = target(node)?;
            screens
                .iter()
                .find(|(held, at, _)| *held == instance && *at == region)
                .map(|(_, _, screen)| *screen)
        };
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        else {
            return (Vec::new(), false);
        };
        let id = screen.request.screen;
        let pressed = input
            .pointer_events()
            .any(|event| matches!(event.phase, beui::PointerPhase::Down(_)));
        let escaped = input.focused
            && input
                .inputs()
                .filter_map(beui::Event::press)
                .any(|press| press.key == beui::Key::Escape && press.pressed);
        let zoom = match screen.request.metrics.logical.width > 0.0 {
            true => input.rect.width() / screen.request.metrics.logical.width,
            false => 1.0,
        };
        let every_sample = screen.window.every_sample;
        let mut messages: Vec<Outbound> = screen
            .input
            .forward(input, id, zoom, every_sample, aim, &screen_of)
            .into_iter()
            .map(Outbound::from)
            .collect();
        let files = super::input::file_drop(input, screen.file_dropping);
        messages.extend(self.file_drop(instance, region, files));
        let revoked =
            (escaped || (pressed && input.hovered)) && self.revoke_active(instance, region);
        (messages, revoked)
    }

    pub(super) fn shown_anchored(
        &self,
        instance: EditorInstanceId,
        region: ViewId,
    ) -> Vec<HostAnchored> {
        self.entries
            .get(&instance)
            .and_then(|entry| {
                let root = entry.screens.get(&region)?;
                Some(host_anchored(root, &entry.anchored, region))
            })
            .unwrap_or_default()
    }

    pub(super) fn adopt_anchored(&mut self, reports: &[FrameReport], layout: &ScreenLayout) {
        for report in reports {
            let Some(placement) = report.anchored else {
                continue;
            };
            let Some(placed) = layout.placement(report.screen).copied() else {
                continue;
            };
            let Some(entry) = self.entries.get_mut(&placed.instance) else {
                continue;
            };
            if let Some(held) = entry
                .anchored
                .iter_mut()
                .find(|held| held.screen.request.screen == report.screen)
            {
                held.placement = placement;
                held.surface = placed.surface;
                continue;
            }
            let root = match entry
                .anchored
                .iter()
                .find(|held| held.view == placement.parent)
            {
                Some(parent) => parent.root,
                None if entry.screens.contains_key(&placement.parent) => placement.parent,
                None => continue,
            };
            if placed.view == root
                || entry.screens.contains_key(&placed.view)
                || entry.anchored.iter().any(|held| held.view == placed.view)
            {
                continue;
            }
            let size = placement.rect.size();
            let opened = LAYERS_OPENED.with(|opened| {
                opened.set(opened.get() + 1);
                opened.get()
            });
            entry.anchored.push(AnchoredScreen {
                view: placed.view,
                root,
                opened,
                placement,
                surface: placed.surface,
                screen: Screen::new(
                    ScreenRequest {
                        screen: report.screen,
                        instance: placed.instance,
                        view: placed.view,
                        placement: ViewPlacement::Anchored,
                        metrics: viewport_metrics(
                            size,
                            Rect::from_min_size(beui::Pos2::ZERO, size),
                            placed.scale_factor(),
                            placed.raster(),
                        ),
                        frame: None,
                        windows: false,
                        host_place: None,
                        origin: None,
                        docked: false,
                        anchor_space: None,
                    },
                    0,
                ),
            });
        }
    }

    pub(super) fn keep_anchored(&mut self, layout: &ScreenLayout) {
        for (instance, entry) in &mut self.entries {
            entry.anchored.retain_mut(|held| {
                let Some(placed) = layout.placement(held.screen.request.screen) else {
                    return false;
                };
                held.surface = placed.surface;
                placed.instance == *instance && placed.view == held.view
            });
            let screens = &entry.screens;
            let mut kept: Vec<ViewId> = Vec::new();
            entry.anchored.retain(|held| {
                let hangs = screens.contains_key(&held.root)
                    && (held.placement.parent == held.root
                        || kept.contains(&held.placement.parent));
                if hangs {
                    kept.push(held.view);
                }
                hangs
            });
        }
    }

    pub(super) fn anchored_views(
        &self,
        instance: EditorInstanceId,
        region: ViewId,
    ) -> Vec<(ViewId, Rect, Rect)> {
        self.shown_anchored(instance, region)
            .into_iter()
            .map(|layer| (layer.id, layer.rect, layer.bounds))
            .collect()
    }

    fn is_anchored_screen(&self, screen: ScreenId) -> bool {
        self.entries.values().any(|entry| {
            entry
                .anchored
                .iter()
                .any(|held| held.screen.request.screen == screen)
        })
    }

    fn any_screen(&self, instance: EditorInstanceId, view: ViewId) -> Option<&Screen> {
        let entry = self.entries.get(&instance)?;
        entry.screens.get(&view).or_else(|| {
            entry
                .anchored
                .iter()
                .find(|held| held.view == view)
                .map(|held| &held.screen)
        })
    }

    fn any_screen_mut(&mut self, instance: EditorInstanceId, view: ViewId) -> Option<&mut Screen> {
        let entry = self.entries.get_mut(&instance)?;
        match entry.screens.get_mut(&view) {
            Some(screen) => Some(screen),
            None => entry
                .anchored
                .iter_mut()
                .find(|held| held.view == view)
                .map(|held| &mut held.screen),
        }
    }

    pub(super) fn forward_anchored(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        layer: ViewId,
        input: &beui::ForwardedInput,
    ) -> Vec<Outbound> {
        let Some(rect) = self
            .shown_anchored(instance, region)
            .into_iter()
            .find(|held| held.id == layer)
            .map(|held| held.rect)
        else {
            return Vec::new();
        };
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.anchored.iter_mut().find(|held| held.view == layer))
            .map(|held| &mut held.screen)
        else {
            return Vec::new();
        };
        let events = input
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    beui::Forwarded::Pointer { .. } | beui::Forwarded::Left
                )
            })
            .cloned()
            .collect();
        let input = beui::ForwardedInput {
            events,
            rect,
            focused: false,
            ..input.clone()
        };
        let every_sample = screen.window.every_sample;
        let id = screen.request.screen;
        screen
            .input
            .forward(
                &input,
                id,
                1.0,
                every_sample,
                &|_, _| beui::Aim::Own,
                &|_| None,
            )
            .into_iter()
            .map(Outbound::from)
            .collect()
    }

    pub(super) fn dismiss_anchored_views(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        dismissed: Vec<ViewId>,
    ) -> Vec<Outbound> {
        let announced = &self.announced;
        let Some(screen) = self
            .entries
            .get(&instance)
            .and_then(|entry| entry.screens.get(&region))
            .filter(|screen| announced.contains(&screen.request.screen))
        else {
            return Vec::new();
        };
        let open: Vec<ViewId> = self
            .entries
            .get(&instance)
            .map(|entry| {
                entry
                    .anchored
                    .iter()
                    .filter(|held| held.root == region)
                    .map(|held| held.view)
                    .collect()
            })
            .unwrap_or_default();
        let dismissed: Vec<ViewId> = dismissed
            .into_iter()
            .filter(|id| open.contains(id))
            .collect();
        if dismissed.is_empty() {
            return Vec::new();
        }
        vec![Outbound::from(PluginCall::Input(
            block_plugin_api::InputBatch {
                screen: screen.request.screen,
                every_sample: false,
                events: Vec::new(),
                dismissed,
            },
        ))]
    }

    pub(super) fn hold(&mut self, instance: EditorInstanceId, region: ViewId) {
        if let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        {
            screen.holding = true;
        }
    }

    pub(super) fn held(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        rect: Option<Rect>,
        clip: Rect,
        drawn: Option<((u32, u32), f32)>,
    ) -> Option<Held> {
        let screen = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))?;
        let stale = drawn
            .filter(|(drawn, finer)| {
                *drawn != super::input::requested_at(&screen.request.metrics, *finer)
            })
            .map(|(drawn, _)| drawn);
        if screen.holding
            && let (Some(drawn), Some((rect, _))) = (stale, screen.presented)
        {
            return Some(Held { rect, drawn });
        }
        screen.holding = false;
        if let Some(rect) = rect {
            screen.presented = Some((rect, clip));
        }
        None
    }

    pub(super) fn set_view(&mut self, instance: EditorInstanceId, view: EditorView) {
        if let Some(entry) = self.entries.get_mut(&instance) {
            entry.view = Some(view);
        }
    }

    pub(super) fn presenting(&self, instance: EditorInstanceId) -> bool {
        self.entries
            .get(&instance)
            .is_some_and(|entry| entry.presenting)
    }

    pub(super) fn set_presenting(&mut self, instance: EditorInstanceId, presenting: bool) -> bool {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        let changed = entry.presenting != presenting;
        entry.presenting = presenting;
        changed
    }

    pub(super) fn set_presented(&mut self, instance: EditorInstanceId, presented: bool) -> bool {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        let changed = entry.presented != presented;
        entry.presented = presented;
        changed
    }

    pub(super) fn assigned_size(&self, instance: EditorInstanceId) -> Option<Vec2> {
        self.entries.get(&instance)?.reported_size
    }

    pub(super) fn resized(&mut self, instance: EditorInstanceId, size: Vec2) -> Vec<Outbound> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        if entry.reported_size.is_some_and(|reported| {
            (reported.x - size.x).abs() < 0.01 && (reported.y - size.y).abs() < 0.01
        }) {
            return Vec::new();
        }
        entry.reported_size = Some(size);
        vec![Outbound::Editor(
            instance,
            EditorCall::Resized(Size {
                width: size.x,
                height: size.y,
            }),
        )]
    }

    pub(super) fn take_bar_actions(
        &mut self,
        instance: EditorInstanceId,
    ) -> Vec<block_plugin_api::BarAction> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.bar_actions))
            .unwrap_or_default()
    }

    pub(super) fn menu(&self, instance: EditorInstanceId) -> Vec<block_plugin_api::MenuEntry> {
        self.entries
            .get(&instance)
            .and_then(|entry| {
                entry
                    .menus
                    .get(&self.content_view(instance))
                    .or_else(|| entry.menus.get(&ViewId::MAIN))
                    .cloned()
            })
            .unwrap_or_default()
    }

    pub(super) fn menu_pick(
        &mut self,
        instance: EditorInstanceId,
        id: String,
        anchor: Option<Rect>,
    ) -> Vec<Outbound> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        let view = entry_menu_holds(self, instance).unwrap_or(ViewId::MAIN);
        vec![Outbound::Editor(
            instance,
            EditorCall::MenuPick { view, id, anchor },
        )]
    }

    pub(super) fn take_child_menu_picks(
        &mut self,
        instance: EditorInstanceId,
        children: &[ChildId],
    ) -> Vec<(ChildId, String, Option<Rect>)> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        let (taken, kept) = std::mem::take(&mut entry.child_menu_picks)
            .into_iter()
            .partition(|(child, _, _)| children.contains(child));
        entry.child_menu_picks = kept;
        taken
    }

    pub(super) fn take_view_changes(&mut self, instance: EditorInstanceId) -> Vec<ViewChange> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.view_changes))
            .unwrap_or_default()
    }

    pub(super) fn report_creation(
        &mut self,
        instance: EditorInstanceId,
        client_id: Uuid,
        block_types: &Arc<crate::PluginCatalog>,
        role: InstanceRole,
    ) -> bool {
        self.connect(client_id);
        if self.block_types.is_none() {
            self.block_types = Some(Arc::clone(block_types));
        }
        self.entries
            .entry(instance)
            .or_insert_with(|| Instance::new(role))
            .opened
    }

    pub(super) fn report_artifact(
        &mut self,
        instance: EditorInstanceId,
        client_id: Uuid,
        block_types: &Arc<crate::PluginCatalog>,
        editor: Uuid,
        block: EditorBlock,
        source: Uuid,
        data: &[u8],
        resync: bool,
    ) -> Vec<Outbound> {
        self.connect(client_id);
        if self.block_types.is_none() {
            self.block_types = Some(Arc::clone(block_types));
        }
        let entry = self.entries.entry(instance).or_insert_with(|| {
            let mut entry = Instance::new(InstanceRole::Artifact(editor, block, source));
            entry.artifact.data = data.to_vec();
            entry
        });
        if entry.artifact.data == data && !resync {
            return Vec::new();
        }
        entry.artifact.data = data.to_vec();
        entry.artifact.draft = None;
        if !entry.opened {
            return Vec::new();
        }
        vec![Outbound::Editor(
            instance,
            EditorCall::ArtifactSettings(data.to_vec()),
        )]
    }

    pub(super) fn artifact_description(
        &self,
        instance: EditorInstanceId,
    ) -> Option<ArtifactDescription> {
        self.entries.get(&instance)?.artifact.description.clone()
    }

    pub(super) fn artifact_draft(&self, instance: EditorInstanceId) -> Option<Vec<u8>> {
        self.entries.get(&instance)?.artifact.draft.clone()
    }

    pub(super) fn regenerate_artifact(
        &mut self,
        instance: EditorInstanceId,
        data: &[u8],
    ) -> Vec<Outbound> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        entry.artifact.outcome = None;
        vec![Outbound::Editor(
            instance,
            EditorCall::RegenerateArtifact(data.to_vec()),
        )]
    }

    pub(super) fn take_artifact_outcome(
        &mut self,
        instance: EditorInstanceId,
    ) -> Option<Result<(), String>> {
        self.entries.get_mut(&instance)?.artifact.outcome.take()
    }

    pub(super) fn allow_network(&mut self, hosts: Vec<String>) {
        self.network = hosts;
    }

    pub(super) fn set_plugin_id(&mut self, plugin_id: String) {
        self.plugin_id = plugin_id;
    }

    pub(super) fn reopen(&mut self) {
        self.sent_block_types = None;
        self.announced.clear();
        for entry in self.entries.values_mut() {
            entry.opened = false;
            entry.deferred.clear();
            entry.reported_focus = None;
            entry.reported_editable = None;
            entry.reported_view = None;
            entry.reported_presenting = false;
            entry.published.forget_reported();
            entry.heads = Heads::default();
            entry.artifact_watch = Some(Vec::new());
            entry.watches.clear();
            entry.replicas.clear();
            entry.shows.clear();
            entry.reported_size = None;
            entry.blocks_seen = None;
            entry.version_sent = None;
            for link in entry.content.iter_mut().chain(entry.watched.values_mut()) {
                link.delivered = false;
            }
            for screen in entry.screens.values_mut() {
                screen.stopped = None;
            }
            entry.stale = true;
        }
        self.forget_services();
        self.owners.clear();
        self.accepted.clear();
        self.epoch += 1;
    }

    pub(super) fn next_screens(&mut self, pass: u64) -> NextScreens {
        let client = self
            .connection
            .as_ref()
            .map(|connection| connection.client_id)
            .map(|client_id| (client_id, block_app_stack::identity().unwrap_or_default()));
        let mut instances: Vec<_> = self.entries.keys().copied().collect();
        instances.sort_by_key(|instance| instance.0);
        let focus = self.focus.clone();
        let graph = block_app_stack::graph_revision();
        let graph_moved = self.graph_seen.replace(graph) != Some(graph);
        let mut opened = Vec::new();
        let mut screens = Vec::new();
        self.relaying.clear();
        let (programs, program_editors) = block_app_base::program_editors::listed();
        let sent = (be_types_host::plain_forms_revision(), programs);
        if self.sent_block_types != Some(sent)
            && let Some(block_types) = &self.block_types
        {
            self.sent_block_types = Some(sent);
            let debug_templates = block_types.debug_templates.clone();
            let mut block_types = block_types.catalog.clone();
            block_types.editors.extend(program_editors);
            for descriptor in &mut block_types.types {
                descriptor.plain_file =
                    be_types_host::plain_of(Uuid::from_bytes(descriptor.block_type))
                        == be_types_host::Plain::File;
            }
            opened.push(Outbound::Plugin(PluginCall::BlockTypes(block_types)));
            opened.push(Outbound::Plugin(PluginCall::DebugTemplates(
                debug_templates,
            )));
        }
        for instance in instances {
            let scope = self.scope(instance);
            let Some(entry) = self.entries.get_mut(&instance) else {
                continue;
            };
            block_app_stack::want(block_app_stack::Owner::Editor(instance.0), entry.wants());
            let docked = super::docks::docked(instance);
            let mut regions: Vec<_> = entry
                .screens
                .values()
                .filter_map(|screen| {
                    let shown = (screen.mounted > 0 || screen.last_seen >= pass)
                        && screen.request.metrics.pixel_width > 0
                        && screen.request.metrics.pixel_height > 0;
                    let frame = screen.request.view.is_main();
                    let mut request = screen.request.clone();
                    request.docked = frame && docked;
                    match (shown, screen.kept) {
                        (true, _) => Some(request),
                        (false, Some(kept)) if request.docked => Some(ScreenRequest {
                            metrics: kept,
                            ..request
                        }),
                        (false, _) => None,
                    }
                })
                .collect();
            if regions.is_empty() && matches!(entry.role, InstanceRole::Editor(..)) {
                continue;
            }
            regions.sort_by_key(|request| request.screen.0);
            let Some((client_id, (account, organization))) = client else {
                continue;
            };
            let client_id = client_id.into_bytes();
            if !entry.opened {
                entry.opened = true;
                let account_id = account.into_bytes();
                let organization_id = organization.into_bytes();
                opened.push(Outbound::Open(Box::new(match entry.role {
                    InstanceRole::Editor(editor, block) => Opening::Block(OpenBlock {
                        instance,
                        editor: editor.into_bytes(),
                        block_id: block.id.into_bytes(),
                        block_type: block.block_type.into_bytes(),
                        view_block: block.view_block.map(Uuid::into_bytes),
                        account_id,
                        organization_id,
                        client_id,
                        editable: {
                            let editable = block_app_stack::access(block.id).can_edit();
                            entry.reported_editable = Some(editable);
                            editable
                        },
                    }),
                    InstanceRole::Picker(editor) => {
                        let picker = super::picker_request(instance).unwrap_or_default();
                        Opening::Picker(OpenPicker {
                            instance,
                            editor: editor.into_bytes(),
                            filter: picker.filter,
                            parent: picker.parent,
                            sheet: picker.sheet,
                            account_id,
                            organization_id,
                            client_id,
                        })
                    }
                    InstanceRole::Rename(editor, block, sheet) => Opening::Rename(OpenRename {
                        instance,
                        editor: editor.into_bytes(),
                        block_id: block.into_bytes(),
                        sheet,
                        account_id,
                        organization_id,
                        client_id,
                    }),
                    InstanceRole::Creation(editor, template) => Opening::Creation(OpenCreation {
                        instance,
                        editor: editor.into_bytes(),
                        template: template.to_owned(),
                        account_id,
                        organization_id,
                        client_id,
                    }),
                    InstanceRole::Artifact(editor, block, source) => {
                        Opening::Artifact(OpenArtifact {
                            instance,
                            editor: editor.into_bytes(),
                            block_id: block.id.into_bytes(),
                            block_type: block.block_type.into_bytes(),
                            source: source.into_bytes(),
                            account_id,
                            organization_id,
                            client_id,
                            data: entry.artifact.data.clone(),
                        })
                    }
                })));
                let mut statuses: Vec<_> = entry
                    .screens
                    .values()
                    .flat_map(|screen| screen.reported_statuses.values().cloned())
                    .collect();
                statuses.sort_by_key(|status| status.child.0);
                if !statuses.is_empty() {
                    opened.push(Outbound::Editor(
                        instance,
                        EditorCall::ChildStatuses(statuses),
                    ));
                }
                let visible = presence_wanted(instance);
                if visible && entry.presence_visible != Some(true) {
                    entry.presence_visible = Some(true);
                    opened.push(Outbound::Editor(instance, EditorCall::Presence(visible)));
                }
            }
            opened.append(&mut entry.deferred);
            self.relaying.insert(instance);
            let stale = std::mem::take(&mut entry.stale);
            if stale || graph_moved {
                entry.reattach(instance);
            }
            if let InstanceRole::Editor(_, block) = entry.role
                && (stale || graph_moved)
            {
                let editable = block_app_stack::access(block.id).can_edit();
                if entry.reported_editable != Some(editable) {
                    entry.reported_editable = Some(editable);
                    opened.push(Outbound::Editor(
                        instance,
                        EditorCall::SetEditable(editable),
                    ));
                }
            }
            entry.offer_blocks(scope);
            if stale {
                entry.offer_version();
            }
            if stale || graph_moved {
                entry.offer_histories();
                let mut heads = Vec::new();
                entry
                    .heads
                    .request(instance, self.epoch, &self.head_reads, |block, head| {
                        heads.push((block, head));
                    });
                for (block, head) in heads {
                    entry.offer(Subject::Head(block), &head);
                }
            }
            if stale || graph_moved {
                entry.name_content();
            }
            if entry.reported_focus.as_ref() != Some(&focus) {
                entry.reported_focus = Some(focus.clone());
                let (block_id, block_type) = match focus.block {
                    Some((block_id, block_type)) => {
                        (Some(block_id.into_bytes()), block_type.into_bytes())
                    }
                    None => (None, [0; 16]),
                };
                opened.push(Outbound::Editor(
                    instance,
                    EditorCall::FocusChanged(block_plugin_api::shell_types::Focus {
                        block_id,
                        block_type,
                        via: focus.via.iter().map(|id| id.into_bytes()).collect(),
                    }),
                ));
            }
            let presenting = entry.presenting || entry.presented;
            if presenting != entry.reported_presenting {
                entry.reported_presenting = presenting;
                opened.push(Outbound::Editor(
                    instance,
                    EditorCall::PresentingChanged(presenting),
                ));
            }
            let mut reported = Vec::new();
            entry.published.report(
                instance,
                self.shell == Some(instance),
                &self.published,
                &mut reported,
            );
            opened.extend(reported);
            if entry.view != entry.reported_view {
                entry.reported_view = entry.view;
                if let Some(view) = entry.view {
                    opened.push(Outbound::Editor(
                        instance,
                        EditorCall::ViewChanged {
                            view: view.rect,
                            scale: view.scale,
                        },
                    ));
                }
            }
            screens.extend(regions);
        }
        NextScreens { opened, screens }
    }

    pub(super) fn configure(&mut self, messages: &[Outbound]) -> Option<PluginCall> {
        let configures = messages.iter().any(|message| {
            matches!(
                message,
                Outbound::Plugin(PluginCall::Screens(_))
                    | Outbound::Editor(_, EditorCall::ViewChanged { .. })
            )
        });
        if !configures {
            return None;
        }
        self.serial += 1;
        let serial = self.serial;
        for message in messages {
            match message {
                Outbound::Plugin(PluginCall::Screens(screens)) => {
                    for request in screens {
                        if let Some(screen) = self.screen_by_id(request.screen) {
                            screen.scales.push(serial, request.metrics.scale_factor);
                            screen.spaces.push(
                                serial,
                                request
                                    .anchor_space
                                    .map(|layers| (layers, request.metrics.raster)),
                            );
                        }
                    }
                }
                Outbound::Editor(instance, EditorCall::ViewChanged { view, scale }) => {
                    if let Some(entry) = self.entries.get_mut(instance) {
                        entry.views.push(
                            serial,
                            EditorView {
                                rect: *view,
                                scale: *scale,
                            },
                        );
                    }
                }
                _ => {}
            }
        }
        Some(PluginCall::Configure(serial))
    }

    pub(super) fn serial(&self) -> u64 {
        self.serial
    }

    fn screen_by_id(&mut self, screen: ScreenId) -> Option<&mut Screen> {
        self.entries
            .values_mut()
            .flat_map(|entry| entry.screens.values_mut())
            .find(|placed| placed.request.screen == screen)
    }

    pub(super) fn confirm(&mut self, serial: u64, held: &HashSet<ScreenId>) -> bool {
        let mut changed = false;
        for entry in self.entries.values_mut() {
            for screen in entry.screens.values_mut() {
                if !held.contains(&screen.request.screen) && screen.serial != serial {
                    changed = true;
                    screen.serial = serial;
                }
            }
        }
        self.forget_confirmed();
        changed
    }

    pub(super) fn laid(&mut self, serial: u64, screens: &HashSet<ScreenId>) {
        for screen in screens {
            if let Some(screen) = self.screen_by_id(*screen) {
                screen.serial = screen.serial.max(serial);
            }
        }
        self.forget_confirmed();
    }

    fn forget_confirmed(&mut self) {
        for entry in self.entries.values_mut() {
            let oldest = entry.screens.values().map(|screen| screen.serial).min();
            for screen in entry.screens.values_mut() {
                screen.scales.confirm(screen.serial);
                screen.spaces.confirm(screen.serial);
            }
            if let Some(oldest) = oldest {
                entry.views.confirm(oldest);
            }
        }
    }

    pub(super) fn set_view_sizes(&mut self, sizes: Vec<ViewSize>) -> bool {
        let mut changed = false;
        for size in sizes {
            if let Some(screen) = self.screen_by_id(size.screen) {
                let used = vec2(size.logical.width, size.logical.height);
                changed |= screen.used != Some(used);
                screen.used = Some(used);
            }
        }
        changed
    }

    pub(super) fn set_windows(&mut self, windows: Vec<ScreenWindow>) -> bool {
        let mut changed = false;
        for ScreenWindow { screen, window } in windows {
            if let Some(screen) = self.screen_by_id(screen) {
                changed |= screen.window != window;
                screen.window = window;
            }
        }
        changed
    }

    pub(super) fn drag(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        event: Option<BlockDragEvent>,
    ) -> Vec<Outbound> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        let Some(screen) = entry.screens.get_mut(&region) else {
            return Vec::new();
        };
        match event {
            Some(event) => {
                screen.dragging = !event.dropped;
                if event.dropped {
                    entry.drag_accepted = false;
                }
                vec![Outbound::Editor(
                    instance,
                    EditorCall::DragOver(block_plugin_api::shell_types::DragOver {
                        view: region,
                        pos: event.position,
                        block_id: event.drag.block_id.into_bytes(),
                        block_type: event.drag.block_type.into_bytes(),
                        dropped: event.dropped,
                    }),
                )]
            }
            None if screen.dragging => {
                screen.dragging = false;
                entry.drag_accepted = false;
                vec![Outbound::Editor(instance, EditorCall::DragLeft)]
            }
            None => Vec::new(),
        }
    }

    pub(super) fn file_drop(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        event: Option<FileDropEvent>,
    ) -> Vec<Outbound> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        let Some(screen) = entry.screens.get_mut(&region) else {
            return Vec::new();
        };
        match event {
            Some(event) => {
                screen.file_dropping = !event.dropped;
                vec![Outbound::Editor(
                    instance,
                    EditorCall::FileDrop(block_plugin_api::shell_types::FileDrop {
                        view: region,
                        pos: event.position,
                        files: event.files,
                        dropped: event.dropped,
                    }),
                )]
            }
            None if screen.file_dropping => {
                screen.file_dropping = false;
                vec![Outbound::Editor(instance, EditorCall::FileDropLeft)]
            }
            None => Vec::new(),
        }
    }

    pub(super) fn dragging(&self, instance: EditorInstanceId, region: ViewId) -> bool {
        self.entries
            .get(&instance)
            .and_then(|entry| entry.screens.get(&region))
            .is_some_and(|screen| screen.dragging)
    }

    pub(super) fn drag_accepted(&self, instance: EditorInstanceId) -> bool {
        self.entries
            .get(&instance)
            .is_some_and(|entry| entry.drag_accepted)
    }

    pub(super) fn set_children(&mut self, placements: ChildPlacements) -> bool {
        let ChildPlacements {
            instance,
            view: region,
            children,
            occluders,
        } = placements;
        let Some(screen) = self.any_screen_mut(instance, region) else {
            return false;
        };
        screen.revoked.retain(|child| {
            children
                .iter()
                .any(|placement| placement.child == *child && placement.mode == ChildMode::Active)
        });
        screen.frame_revoked.retain(|child| {
            children.iter().any(|placement| {
                placement.child == *child
                    && matches!(placement.mode, ChildMode::Active | ChildMode::Live)
            })
        });
        let table = ChildTable {
            children,
            occluders,
        };
        let changed = screen.children != table;
        screen.children = table;
        changed
    }

    pub(super) fn only_moves(&self, placements: &ChildPlacements) -> bool {
        let Some(screen) = self.any_screen(placements.instance, placements.view) else {
            return false;
        };
        let current = &screen.children.children;
        current.len() == placements.children.len()
            && current
                .iter()
                .zip(&placements.children)
                .all(|(current, next)| {
                    (
                        current.child,
                        current.mode,
                        current.own_frame,
                        current.top_bar,
                        current.layer,
                        current.intrinsic,
                    ) == (
                        next.child,
                        next.mode,
                        next.own_frame,
                        next.top_bar,
                        next.layer,
                        next.intrinsic,
                    )
                })
    }

    pub(super) fn host_children(
        &self,
        instance: EditorInstanceId,
        region: ViewId,
        rect: Rect,
        clip: Rect,
        view: Option<EditorView>,
    ) -> (Vec<HostChild>, Holes) {
        let Some((entry, screen)) = self
            .entries
            .get(&instance)
            .and_then(|entry| Some((entry, entry.screens.get(&region)?)))
        else {
            return (Vec::new(), Holes::default());
        };
        let table = &screen.children;
        let origin = rect.min.to_vec2();
        let scale = screen.shown_scale(rect);
        let laid_view = entry.views.at(screen.serial).copied();
        let mut children = Vec::new();
        let mut holes = Holes::default();
        let mut live = 0;
        let mut shown_tabs = HashSet::default();
        let layers = host_anchored(screen, &entry.anchored, region);
        if let Some(report) = &screen.report {
            let painted: Vec<Rect> = report
                .painted
                .iter()
                .map(|painted| host_rect(*painted, origin, scale))
                .collect();
            if !painted.is_empty() {
                for piece in pieces::subtract(rect.intersect(clip), &painted) {
                    holes.holes.push(Hole {
                        rect: piece,
                        occluders: Vec::new(),
                    });
                }
            }
        }
        let in_layers = entry.anchored.iter().filter_map(|held| {
            let layer = layers.iter().find(|layer| layer.id == held.view)?;
            let from = layer.rect.min.to_vec2();
            let moved = layer.rect.min - held.placement.rect.min;
            Some(held.screen.children.children.iter().map(move |child| {
                let mut child = child.clone();
                child.rect = child.rect.translate(from);
                child.clip = child.clip.translate(from);
                child.anchor = child.anchor.map(|anchor| anchor.translate(moved));
                (usize::MAX, child, Some(layer))
            }))
        });
        let placed: Vec<(usize, ChildPlacement, Option<&HostAnchored>)> = table
            .children
            .iter()
            .enumerate()
            .map(|(index, child)| (index, child.clone(), None))
            .chain(in_layers.flatten())
            .collect();
        let mut keyboards = Vec::new();
        for (index, child, layer) in placed {
            let child = &child;
            if self
                .entries
                .get(&instance)
                .and_then(|entry| entry.role.block())
                .is_some_and(|block| block_app_stack::revisions::source(block.id).is_some())
                && !matches!(
                    child.content,
                    ChildContent::Block { .. } | ChildContent::WebView(_)
                )
            {
                continue;
            }
            let content = match &child.content {
                ChildContent::Block {
                    block_id,
                    block_type,
                    view_block,
                    editor,
                } => super::HostContent::Block {
                    block_id: self
                        .entries
                        .get(&instance)
                        .and_then(|entry| entry.role.block())
                        .map_or_else(
                            || Uuid::from_bytes(*block_id),
                            |parent| {
                                block_app_stack::revisions::alias(
                                    parent.id,
                                    Uuid::from_bytes(*block_id),
                                )
                            },
                        ),
                    block_type: Uuid::from_bytes(*block_type),
                    view_block: if self
                        .entries
                        .get(&instance)
                        .and_then(|entry| entry.role.block())
                        .is_some_and(|parent| {
                            block_app_stack::revisions::source(parent.id).is_some()
                        }) {
                        None
                    } else {
                        view_block.map(Uuid::from_bytes)
                    },
                    editor: editor.map(Uuid::from_bytes),
                },
                ChildContent::Host(panel) => super::HostContent::Panel(*panel),
                ChildContent::Window(window) => super::HostContent::Window(*window),
                ChildContent::Creation { editor, template } => super::HostContent::Creation {
                    editor: Uuid::from_bytes(*editor),
                    template: template.clone(),
                },
                ChildContent::ArtifactSettings { block_id } => {
                    super::HostContent::ArtifactSettings(Uuid::from_bytes(*block_id))
                }
                ChildContent::DockTab(path) => match super::docks::resolve(instance, path) {
                    Some((block, tab)) if shown_tabs.insert((block, tab)) => {
                        super::HostContent::DockTab { block, tab }
                    }
                    _ => continue,
                },
                ChildContent::Picker { filter, parent } => super::HostContent::Picker {
                    filter: filter.clone(),
                    parent: *parent,
                },
                ChildContent::Rename(block) => super::HostContent::Rename(Uuid::from_bytes(*block)),
                ChildContent::WebView(_) => continue,
            };
            if !child.rect.is_positive() {
                continue;
            }
            let requested = match (region, child.mode) {
                (ViewId::PREVIEW, _) => ChildMode::Preview,
                (_, ChildMode::Active) if screen.revoked.contains(&child.child) => {
                    ChildMode::Passive
                }
                (_, mode) => mode,
            };
            let mode = match requested {
                ChildMode::Preview => ChildMode::Preview,
                mode => {
                    live += 1;
                    match live > MAX_LIVE_CHILDREN {
                        true => ChildMode::Preview,
                        false => mode,
                    }
                }
            };
            let anchor = child.anchor.map(|anchor| match layer {
                Some(_) => anchor,
                None => host_rect(anchor, origin, scale),
            });
            let (child_rect, view_rect, child_clip, occluders) = match layer {
                Some(layer) => {
                    let rect = child.rect;
                    let clip = child.clip.intersect(layer.rect).intersect(layer.bounds);
                    (rect, rect, clip, Vec::new())
                }
                None => {
                    let child_rect = host_rect(child.rect, origin, scale);
                    let (view_rect, child_clip) = match (child.follows_view, laid_view, view) {
                        (true, Some(laid_view), Some(view)) => (
                            host_rect(through(child.rect, laid_view, view), origin, scale),
                            host_rect(followed_clip(child, laid_view, view), origin, scale),
                        ),
                        _ => (child_rect, host_rect(child.clip, origin, scale)),
                    };
                    let occluders: Vec<Rect> = table
                        .occluders
                        .iter()
                        .filter(|occluder| occluder.after as usize > index)
                        .map(|occluder| host_rect(occluder.rect, origin, scale))
                        .collect();
                    (child_rect, view_rect, child_clip.intersect(clip), occluders)
                }
            };
            if layer.is_none()
                && matches!(
                    mode,
                    ChildMode::Active | ChildMode::Live | ChildMode::InPlace
                )
            {
                let interactive = child_rect.intersect(child_clip);
                if interactive.is_positive() {
                    holes.holes.push(Hole {
                        rect: interactive,
                        occluders: occluders.clone(),
                    });
                }
            }
            children.push(HostChild {
                child: child.child,
                frame_owner: matches!(mode, ChildMode::Active | ChildMode::Live)
                    && !screen.frame_revoked.contains(&child.child),
                own_frame: child.own_frame,
                top_bar: child.top_bar,
                content,
                rect: child_rect,
                view_rect,
                occluders,
                clip: child_clip,
                layer: child.layer,
                mode,
                intrinsic: child.intrinsic.map(|size| vec2(size.width, size.height)),
                rotation: child.rotation,
                opacity: child.opacity,
                corner_radius: match layer {
                    Some(_) => child.corner_radius,
                    None => child.corner_radius * scale.x.min(scale.y),
                },
                dialog: child.dialog,
                anchored: layer.map(|layer| layer.id),
                keyboard: layer.is_some_and(|layer| {
                    layer.keyboard && {
                        let first = !keyboards.contains(&layer.id);
                        keyboards.push(layer.id);
                        first
                    }
                }),
                sheet: layer.is_some_and(|layer| layer.edge),
                anchor,
            });
        }
        (children, holes)
    }

    pub(super) fn revoke_active(&mut self, instance: EditorInstanceId, region: ViewId) -> bool {
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        else {
            return false;
        };
        let mut revoked = false;
        for child in &screen.children.children {
            if child.mode == ChildMode::Active {
                revoked |= screen.revoked.insert(child.child);
            }
        }
        revoked
    }

    pub(super) fn take_leaving(&mut self, instance: EditorInstanceId) -> bool {
        self.entries
            .get_mut(&instance)
            .is_some_and(|entry| std::mem::take(&mut entry.leaving))
    }

    pub(super) fn content_view(&self, instance: EditorInstanceId) -> ViewId {
        let Some(entry) = self.entries.get(&instance) else {
            return ViewId::MAIN;
        };
        let holds = |view: &ViewId| {
            entry.screens.get(view).is_some_and(|screen| {
                screen.mounted > 0
                    && screen
                        .report
                        .as_ref()
                        .is_some_and(|report| report.content.is_some())
            })
        };
        if holds(&ViewId::MAIN) {
            return ViewId::MAIN;
        }
        entry
            .screens
            .iter()
            .filter(|(_, screen)| screen.request.placement == ViewPlacement::Dock)
            .map(|(view, _)| *view)
            .find(holds)
            .unwrap_or(ViewId::MAIN)
    }

    pub(super) fn frame_child(&self, instance: EditorInstanceId) -> Option<Uuid> {
        self.framing(instance).map(|(_, _, block)| block)
    }

    fn framing(&self, instance: EditorInstanceId) -> Option<(ViewId, ChildId, Uuid)> {
        let view = self.content_view(instance);
        let screen = self.entries.get(&instance)?.screens.get(&view)?;
        screen
            .children
            .children
            .iter()
            .filter(|child| {
                matches!(child.mode, ChildMode::Active | ChildMode::Live)
                    && !child.own_frame
                    && !screen.frame_revoked.contains(&child.child)
                    && !screen.revoked.contains(&child.child)
                    && child.rect.is_positive()
            })
            .find_map(|child| {
                Some((
                    view,
                    child.child,
                    Uuid::from_bytes(child.content.block_id()?),
                ))
            })
    }

    pub(super) fn frame_children(
        &self,
    ) -> Vec<(EditorInstanceId, Option<super::docks::DockedChild>)> {
        self.entries
            .keys()
            .map(|instance| {
                let framed =
                    self.framing(*instance)
                        .map(|(view, child, block)| super::docks::DockedChild {
                            child,
                            tab: view.0,
                            block,
                        });
                (*instance, framed)
            })
            .collect()
    }

    pub(super) fn revoke_frame_child(&mut self, instance: EditorInstanceId) {
        let view = self.content_view(instance);
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&view))
        else {
            return;
        };
        for child in &screen.children.children {
            if matches!(child.mode, ChildMode::Active | ChildMode::Live) && !child.own_frame {
                screen.frame_revoked.insert(child.child);
            }
        }
    }

    pub(super) fn set_child_statuses(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        statuses: Vec<HostChildStatus>,
    ) -> Vec<Outbound> {
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        else {
            return Vec::new();
        };
        let mut changed = Vec::new();
        let live: HashSet<ChildId> = statuses.iter().map(|status| status.child).collect();
        for status in statuses {
            let status = ChildStatus {
                instance,
                view: region,
                child: status.child,
                available: status.available,
                intrinsic: status.intrinsic.map(|size| Size {
                    width: size.x,
                    height: size.y,
                }),
                aspect_ratio: status.aspect_ratio,
                active: status.active,
                interaction: status.interaction,
                capabilities: status.capabilities,
                resize: status.resize,
                error: status.error,
                menu: MenuEntry::flatten(&status.menu),
                creation: status.creation,
                preview: status.preview,
                dialog: status.dialog,
                picked: status.picked.clone(),
                card: status.card,
            };
            if screen.reported_statuses.get(&status.child) == Some(&status) {
                continue;
            }
            screen
                .reported_statuses
                .insert(status.child, status.clone());
            changed.push(status);
        }
        screen
            .reported_statuses
            .retain(|child, _| live.contains(child));
        match changed.is_empty() {
            true => Vec::new(),
            false => vec![Outbound::Editor(
                instance,
                EditorCall::ChildStatuses(changed),
            )],
        }
    }

    pub(super) fn picked(&self, instance: EditorInstanceId) -> Option<BlockPick> {
        self.entries.get(&instance)?.picked.clone()
    }

    pub(super) fn card(&self, instance: EditorInstanceId) -> bool {
        self.entries.get(&instance).is_some_and(|entry| entry.card)
    }

    pub(super) fn take_child_dismissals(&mut self, instance: EditorInstanceId) -> Vec<ChildId> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.child_dismissals))
            .unwrap_or_default()
    }

    pub(super) fn take_child_commits(&mut self, instance: EditorInstanceId) -> Vec<ChildCommit> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.child_commits))
            .unwrap_or_default()
    }

    pub(super) fn statuses(&self, layout: &ScreenLayout, pass: u64) -> Vec<super::InstanceStatus> {
        let mut statuses: Vec<_> = self
            .entries
            .iter()
            .map(|(instance, entry)| {
                let mut screens: Vec<_> = entry
                    .screens
                    .values()
                    .map(|screen| {
                        let metrics = &screen.request.metrics;
                        let placement = layout
                            .screens
                            .iter()
                            .find(|placement| placement.screen == screen.request.screen)
                            .map(|placement| {
                                [placement.surface, placement.width, placement.height]
                            });
                        super::ScreenStatus {
                            screen: screen.request.screen,
                            view: screen.request.view,
                            logical: vec2(metrics.logical.width, metrics.logical.height),
                            pixels: [metrics.pixel_width, metrics.pixel_height],
                            scale_factor: metrics.scale_factor,
                            raster: metrics.raster,
                            used: screen.used,
                            placement,
                            drawn: screen.mounted > 0 || screen.last_seen >= pass,
                            children: screen.children.children.len(),
                            serial: screen.serial,
                        }
                    })
                    .collect();
                screens.sort_by_key(|screen| screen.screen.0);
                super::InstanceStatus {
                    instance: *instance,
                    block: entry.role.block().map(|block| block.id),
                    role: match entry.role {
                        InstanceRole::Editor(..) => "editor",
                        InstanceRole::Creation(..) => "creation",
                        InstanceRole::Picker(..) => "picker",
                        InstanceRole::Rename(..) => "rename",
                        InstanceRole::Artifact(..) => "artifact",
                    },
                    opened: entry.opened,
                    aspect_ratio: entry.aspect_ratio,
                    intrinsic: entry.intrinsic,
                    view: entry.view.map(|view| view.rect),
                    artifact: matches!(entry.role, InstanceRole::Artifact(..)).then(|| {
                        super::ArtifactStatus {
                            data: entry.artifact.data.len(),
                            draft: entry.artifact.draft.as_ref().map(Vec::len),
                            description: entry.artifact.description.as_ref().map(|description| {
                                match description {
                                    ArtifactDescription::Described(summary) => summary.clone(),
                                    ArtifactDescription::Unreadable(error) => {
                                        format!("unreadable: {error}")
                                    }
                                }
                            }),
                        }
                    }),
                    screens,
                }
            })
            .collect();
        statuses.sort_by_key(|status| status.instance.0);
        statuses
    }

    pub(super) fn dialog_status(&self, instance: EditorInstanceId) -> Option<DialogStatus> {
        self.entries.get(&instance).map(|entry| entry.dialog)
    }

    pub(super) fn forget_dialog_answer(&mut self, instance: EditorInstanceId) {
        if let Some(entry) = self.entries.get_mut(&instance) {
            entry.dialog.answer = None;
        }
    }

    pub(super) fn dismiss(&self, instance: EditorInstanceId) -> Vec<Outbound> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Outbound::Editor(instance, EditorCall::Dismiss)]
    }

    pub(super) fn commit_creation(&self, instance: EditorInstanceId) -> Vec<Outbound> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Outbound::Editor(instance, EditorCall::CommitCreation)]
    }

    pub(super) fn take_created(
        &mut self,
        instance: EditorInstanceId,
    ) -> Option<Result<Uuid, String>> {
        self.entries.get_mut(&instance)?.created.take()
    }

    pub(super) fn aspect_ratio(&self, instance: EditorInstanceId) -> Option<f32> {
        self.entries.get(&instance)?.aspect_ratio
    }

    pub(super) fn intrinsic_size(&self, instance: EditorInstanceId) -> Option<Vec2> {
        self.entries.get(&instance)?.intrinsic
    }

    pub(super) fn view_size(&self, instance: EditorInstanceId, region: ViewId) -> Option<Vec2> {
        self.entries.get(&instance)?.screens.get(&region)?.used
    }

    pub(super) fn adopt_window(&mut self, open: &WindowOpen, layout: &ScreenLayout) {
        let instance = open.opener.instance;
        let Some(placed) = layout
            .placement(open.screen)
            .filter(|placed| placed.instance == instance && placed.view == open.view)
        else {
            return;
        };
        let Some(entry) = self.entries.get_mut(&instance) else {
            return;
        };
        if entry
            .screens
            .get(&open.view)
            .is_some_and(|screen| screen.request.screen == open.screen || screen.mounted > 0)
        {
            return;
        }
        let size = vec2(open.size.width, open.size.height);
        let shown = Rect::from_min_size(beui::Pos2::ZERO, size);
        entry.screens.insert(
            open.view,
            Screen::new(
                ScreenRequest {
                    screen: open.screen,
                    instance,
                    view: open.view,
                    placement: ViewPlacement::Host(HostView::Window),
                    metrics: viewport_metrics(size, shown, placed.scale_factor(), placed.raster()),
                    frame: None,
                    windows: true,
                    host_place: None,
                    origin: None,
                    docked: false,
                    anchor_space: None,
                },
                0,
            ),
        );
    }

    pub(super) fn set_frame_reports(&mut self, reports: Vec<FrameReport>) -> bool {
        let mut changed = false;
        for mut report in reports {
            let mut tree = report
                .description
                .as_mut()
                .and_then(|description| description.tree.take())
                .map(beui::accesskit::TreeUpdate::from);
            for entry in self.entries.values_mut() {
                let anchored = entry.anchored.iter_mut().map(|held| &mut held.screen);
                if let Some(screen) = entry
                    .screens
                    .values_mut()
                    .chain(anchored)
                    .find(|screen| screen.request.screen == report.screen)
                {
                    if let Some(tree) = tree.take() {
                        screen.accessibility.update(tree);
                        changed = true;
                    }
                    changed |= screen.report.as_ref() != Some(&report);
                    screen.report = Some(report.clone());
                }
            }
        }
        changed
    }

    pub(super) fn frame_report(
        &self,
        instance: EditorInstanceId,
        region: ViewId,
    ) -> Option<&FrameReport> {
        self.entries
            .get(&instance)?
            .screens
            .get(&region)?
            .report
            .as_ref()
    }

    pub(super) fn described(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
    ) -> Option<Described<'_>> {
        let screen = self.any_screen_mut(instance, region)?;
        let description = screen.report.as_ref()?.description.as_ref()?;
        Some(Described {
            screen: screen.request.screen,
            actions: &description.actions,
            focused: screen.input.focused(),
            tree: &mut screen.accessibility,
        })
    }

    pub(super) fn request_focus(&mut self, screen: ScreenId) {
        for entry in self.entries.values_mut() {
            for placed in entry.screens.values_mut() {
                if placed.request.screen == screen {
                    placed.focus_requested = true;
                }
            }
        }
    }

    pub(super) fn take_focus_request(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
    ) -> bool {
        self.entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
            .is_some_and(|screen| std::mem::take(&mut screen.focus_requested))
    }

    pub(super) fn drive_web_views(&mut self) -> Vec<Outbound> {
        let mut messages = Vec::new();
        for (instance, entry) in &mut self.entries {
            for web_view in entry.unshown_web_views.drain(..) {
                messages.push(Outbound::Editor(
                    *instance,
                    EditorCall::WebViewEvent {
                        web_view,
                        event: WebViewEvent::Failed(
                            "This app can't show web pages yet.".to_owned(),
                        ),
                    },
                ));
            }
        }
        messages
    }

    pub(super) fn set_presence_visible(
        &mut self,
        instance: EditorInstanceId,
        visible: bool,
    ) -> Vec<Outbound> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        if entry.presence_visible == Some(visible) {
            return Vec::new();
        }
        entry.presence_visible = Some(visible);
        vec![Outbound::Editor(instance, EditorCall::Presence(visible))]
    }

    pub(super) fn child_bar_actions(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        actions: Vec<(ChildId, block_plugin_api::BarAction)>,
    ) -> Vec<Outbound> {
        actions
            .into_iter()
            .map(|(child, action)| {
                Outbound::Editor(
                    instance,
                    EditorCall::ChildBar {
                        view: region,
                        child,
                        action,
                    },
                )
            })
            .collect()
    }

    pub(super) fn replace_child(
        &mut self,
        instance: EditorInstanceId,
        old: Uuid,
        new: Uuid,
    ) -> (Vec<Outbound>, Option<bool>) {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return (Vec::new(), None);
        };
        match entry.replacements.get(&(old, new)) {
            Some(Replacement::Pending(_)) => (Vec::new(), None),
            Some(Replacement::Answered(_)) => {
                let Some(Replacement::Answered(replaced)) = entry.replacements.remove(&(old, new))
                else {
                    unreachable!("the replacement was just answered")
                };
                (Vec::new(), Some(replaced))
            }
            None => {
                entry.next_replacement += 1;
                let request_id = entry.next_replacement;
                entry
                    .replacements
                    .insert((old, new), Replacement::Pending(request_id));
                (
                    vec![Outbound::Editor(
                        instance,
                        EditorCall::ReplaceChild {
                            request_id,
                            old: old.into_bytes(),
                            new: new.into_bytes(),
                        },
                    )],
                    None,
                )
            }
        }
    }

    pub(super) fn grabbing(&self) -> bool {
        self.entries
            .values()
            .flat_map(|entry| entry.screens.values())
            .any(|screen| screen.window.pointer_locked)
    }

    pub(super) fn ime(
        &self,
        instance: EditorInstanceId,
        region: ViewId,
        rect: Rect,
    ) -> Option<ImeArea> {
        let screen = self.entries.get(&instance)?.screens.get(&region)?;
        let area = screen.window.ime.as_ref()?;
        let origin = rect.min.to_vec2();
        let scale = screen.shown_scale(rect);
        Some(ImeArea {
            rect: host_rect(area.rect, origin, scale),
            cursor: host_rect(area.cursor, origin, scale),
            text: area.text.clone(),
            keyboard: area.keyboard,
        })
    }

    pub(super) fn window(
        &self,
        instance: EditorInstanceId,
        region: ViewId,
    ) -> Option<&WindowRequests> {
        let screen = self.entries.get(&instance)?.screens.get(&region)?;
        Some(&screen.window)
    }

    pub(super) fn cursor(
        &self,
        instance: EditorInstanceId,
        region: ViewId,
    ) -> Option<beui::CursorIcon> {
        let screen = self.entries.get(&instance)?.screens.get(&region)?;
        Some(screen.window.cursor)
    }

    pub(super) fn screen_set(&mut self, screens: Vec<ScreenRequest>) -> Vec<ScreenRequest> {
        self.announced = screens.iter().map(|screen| screen.screen).collect();
        screens
    }

    fn is_screen(&self, id: ScreenId) -> bool {
        self.entries
            .values()
            .flat_map(|entry| entry.screens.values())
            .any(|screen| screen.request.screen == id)
    }

    pub(super) fn take_announced_input(&mut self) -> Vec<Outbound> {
        let held = std::mem::take(&mut self.unannounced_input);
        let (announced, waiting): (Vec<Outbound>, Vec<Outbound>) =
            held.into_iter().partition(|message| match message {
                Outbound::Plugin(PluginCall::Input(batch)) => {
                    self.announced.contains(&batch.screen)
                }
                _ => true,
            });
        self.unannounced_input = waiting
            .into_iter()
            .filter(|message| match message {
                Outbound::Plugin(PluginCall::Input(batch)) => self.is_screen(batch.screen),
                _ => false,
            })
            .collect();
        announced
    }

    pub(super) fn place(
        &mut self,
        instance: EditorInstanceId,
        region: ViewId,
        placement: Placement,
    ) {
        if let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        {
            screen.placement = Some(placement);
        }
    }

    pub(super) fn touch(&mut self, blocks: &HashSet<Uuid>) {
        if blocks.is_empty() {
            return;
        }
        for entry in self.entries.values_mut() {
            if !entry.stale && blocks.iter().any(|block| entry.follows(*block)) {
                entry.stale = true;
            }
        }
    }

    pub(super) fn pending(&mut self) -> Vec<Outbound> {
        let entries = &mut self.entries;
        self.head_reads
            .arrived(self.epoch, |instance, block, commit, head| {
                if let Some(entry) = entries.get_mut(&instance)
                    && entry.heads.wants(block, commit)
                {
                    entry.offer(Subject::Head(block), &head);
                }
            });
        let mut messages: Vec<Outbound> = Vec::new();
        let mut played: Vec<_> = self.audio_changes.receiver.try_iter().collect();
        played.sort_by_key(|instance| instance.0);
        played.dedup();
        for instance in played {
            if let Some(player) = self
                .entries
                .get(&instance)
                .and_then(|entry| entry.audio.as_ref())
            {
                messages.push(Outbound::Editor(
                    instance,
                    EditorCall::AudioStatus(player.status()),
                ));
            }
        }
        messages
    }

    pub(super) fn watch_published(
        &mut self,
        instance: EditorInstanceId,
        open: block_plugin_api::ops::MethodId,
    ) -> bool {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        entry.published.watch(open);
        true
    }

    pub(super) fn published_call(
        &mut self,
        instance: EditorInstanceId,
        called: block_plugin_api::ops::Called,
    ) -> bool {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        let preview = entry
            .role
            .block()
            .is_some_and(|block| block_app_stack::revisions::source(block.id).is_some());
        !preview && entry.published.call(called)
    }

    pub(super) fn shell_call(&mut self, call: ShellCall) -> bool {
        let Some(instance) = calls::instance_of(&call) else {
            return false;
        };
        let preview = self
            .entries
            .get(&instance)
            .and_then(|entry| entry.role.block())
            .is_some_and(|block| block_app_stack::revisions::source(block.id).is_some());
        if preview
            && !matches!(
                &call,
                ShellCall::Menu { .. }
                    | ShellCall::WebViewNew { .. }
                    | ShellCall::Focused { .. }
                    | ShellCall::ChangeView { .. }
                    | ShellCall::ViewStopped { .. }
                    | ShellCall::CopyText { .. }
                    | ShellCall::CopyImage { .. }
            )
        {
            return false;
        }
        if let ShellCall::Present { presenting, .. } = call {
            return self.set_presenting(instance, presenting);
        }
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        match call {
            ShellCall::RunBarAction { action, .. } => {
                if !super::docks::bar(instance, action) {
                    entry.bar_actions.push(action);
                }
            }
            ShellCall::Menu { view, items, .. } => match MenuEntry::nest(items.clone()) {
                Ok(menu) => {
                    match menu.is_empty() {
                        true => entry.menus.remove(&view),
                        false => entry.menus.insert(view, menu),
                    };
                    super::docks::set_menu(instance, view, items);
                }
                Err(_) => return false,
            },
            ShellCall::ChildMenuPick {
                child, id, anchor, ..
            } => entry.child_menu_picks.push((child, id, anchor)),
            ShellCall::CopyText { text, .. } => {
                host::copy_text(text);
                return false;
            }
            ShellCall::CopyImage { png, .. } => {
                host::copy_image(png);
                return false;
            }
            ShellCall::Paste { .. } => {
                host::request_paste();
                return false;
            }
            ShellCall::DragBlock {
                block_id,
                block_type,
                ..
            } => entry.block_drags.push(BlockDrag {
                block_id: Uuid::from_bytes(block_id),
                block_type: Uuid::from_bytes(block_type),
            }),
            ShellCall::DragAccepted { accepted, .. } => {
                let changed = entry.drag_accepted != accepted;
                entry.drag_accepted = accepted;
                return changed;
            }
            ShellCall::AspectRatio { ratio, .. } => {
                let changed = entry.aspect_ratio != ratio;
                entry.aspect_ratio = ratio;
                return changed;
            }
            ShellCall::IntrinsicSize { size, .. } => {
                let intrinsic = size.map(|size| vec2(size.width, size.height));
                let changed = entry.intrinsic != intrinsic;
                entry.intrinsic = intrinsic;
                return changed;
            }
            ShellCall::Act { block, command, .. } => entry
                .shell_commands
                .push((Uuid::from_bytes(block), command)),
            ShellCall::LeaveFrame { .. } => entry.leaving = true,
            ShellCall::ViewStopped { view, stopped, .. } => {
                let Some(screen) = entry.screens.get_mut(&view) else {
                    return false;
                };
                screen.stopped = Some(stopped);
            }
            ShellCall::ChangeView { change, .. } => entry.view_changes.push(change),
            ShellCall::Focused { focus, .. } => entry.focus_reports.push(Focus {
                block: focus.block_id.map(|block_id| {
                    (
                        Uuid::from_bytes(block_id),
                        Uuid::from_bytes(focus.block_type),
                    )
                }),
                via: focus.via.into_iter().map(Uuid::from_bytes).collect(),
            }),
            ShellCall::HandlesDismiss { handles, .. } => {
                let changed = entry.dialog.handles_dismiss != handles;
                entry.dialog.handles_dismiss = handles;
                return changed;
            }
            ShellCall::CloseDialog { answer, .. } => {
                if entry.dialog.answer.is_some() {
                    return false;
                }
                entry.dialog.answer = Some(answer);
            }
            ShellCall::DismissChild { child, .. } => entry.child_dismissals.push(child),
            ShellCall::ShowCard { card, .. } => {
                if entry.card == card {
                    return false;
                }
                entry.card = card;
            }
            ShellCall::Picked { pick, .. } => {
                if entry.picked.is_some() {
                    return false;
                }
                entry.picked = Some(pick);
            }
            ShellCall::CommitChild {
                child,
                parent,
                name,
                ..
            } => entry.child_commits.push(ChildCommit {
                child,
                parent: block_app_stack::plugin_graph::parent_of(parent),
                name,
            }),
            ShellCall::ArtifactDescribed { description, .. } => {
                entry.artifact.description = Some(description);
            }
            ShellCall::ArtifactEdited { data, .. } => {
                let changed = entry.artifact.draft.as_ref() != Some(&data);
                entry.artifact.draft = Some(data);
                return changed;
            }
            ShellCall::PlayAudio { command, .. } => {
                let changes = self.audio_changes.sender.clone();
                let player = entry.audio.get_or_insert_with(|| {
                    AudioPlayer::new(move || {
                        let _ = changes.send(instance);
                    })
                });
                match command {
                    AudioCommand::Load(source) => player.load(source),
                    AudioCommand::Toggle => player.toggle(),
                    AudioCommand::Reset => player.reset(),
                }
            }
            ShellCall::WebViewNew { id, .. } => entry.unshown_web_views.push(id),
            ShellCall::Present { .. }
            | ShellCall::AudioStatusWatchNew { .. }
            | ShellCall::AudioStatusWatchNext { .. }
            | ShellCall::WebViewLoad { .. }
            | ShellCall::WebViewReload { .. }
            | ShellCall::WebViewFocusApp { .. }
            | ShellCall::WebViewNextEvents { .. }
            | ShellCall::ChildStatusesWatchNew { .. }
            | ShellCall::ChildStatusesWatchNext { .. }
            | ShellCall::BlockTypesWatchNew { .. }
            | ShellCall::BlockTypesWatchNext { .. } => return false,
        }
        true
    }

    pub(super) fn answered(&mut self, instance: EditorInstanceId, answer: EditorAnswer) -> bool {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        match answer {
            EditorAnswer::Created(created) => {
                entry.created = Some(
                    created
                        .map(Uuid::from_bytes)
                        .map_err(|failure| failure.to_string()),
                )
            }
            EditorAnswer::Regenerated(outcome) => {
                entry.artifact.outcome = Some(outcome.map_err(|failure| failure.to_string()))
            }
            EditorAnswer::Replaced {
                request_id,
                replaced,
            } => {
                let Some(key) = entry
                    .replacements
                    .iter()
                    .find(
                        |(_, state)| matches!(state, Replacement::Pending(id) if *id == request_id),
                    )
                    .map(|(key, _)| *key)
                else {
                    return false;
                };
                entry
                    .replacements
                    .insert(key, Replacement::Answered(replaced));
            }
        }
        true
    }

    pub(super) fn take_open(&mut self, instance: EditorInstanceId) -> Option<OpenRequest> {
        let entry = self.entries.get_mut(&instance)?;
        if entry.opens.is_empty() {
            None
        } else {
            Some(entry.opens.remove(0))
        }
    }

    pub(super) fn take_block_drags(&mut self, instance: EditorInstanceId) -> Vec<BlockDrag> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.block_drags))
            .unwrap_or_default()
    }

    pub(super) fn debug_call(&mut self, instance: EditorInstanceId, call: DebugCall) -> bool {
        match call {
            DebugCall::Performance {
                group,
                measurements,
            } => {
                for measurement in measurements {
                    match measurement {
                        PerformanceMeasurement::Duration { name, nanoseconds } => {
                            performance::record_group_duration(
                                &group,
                                &name,
                                std::time::Duration::from_nanos(nanoseconds),
                            );
                        }
                        PerformanceMeasurement::Count { name, count } => {
                            performance::record_group_count(&group, &name, count);
                        }
                    }
                }
                false
            }
            DebugCall::SimulateAccess { block, access } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.simulated.push((Uuid::from_bytes(block), access));
                true
            }
            DebugCall::DebugTemplatesWatchNew { .. }
            | DebugCall::DebugTemplatesWatchNext { .. } => false,
        }
    }

    pub(super) fn take_shell_commands(
        &mut self,
        instance: EditorInstanceId,
    ) -> Vec<(Uuid, ShellCommand)> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.shell_commands))
            .unwrap_or_default()
    }

    pub(super) fn take_simulated_access(
        &mut self,
        instance: EditorInstanceId,
    ) -> Vec<(Uuid, AccessLevel)> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.simulated))
            .unwrap_or_default()
    }

    pub(super) fn take_block_command(
        &mut self,
        instance: EditorInstanceId,
    ) -> Option<(Uuid, BlockCommand)> {
        let entry = self.entries.get_mut(&instance)?;
        if entry.block_commands.is_empty() {
            None
        } else {
            Some(entry.block_commands.remove(0))
        }
    }

    pub(super) fn take_focus_report(&mut self, instance: EditorInstanceId) -> Option<Focus> {
        let entry = self.entries.get_mut(&instance)?;
        if entry.focus_reports.is_empty() {
            None
        } else {
            Some(entry.focus_reports.remove(0))
        }
    }

    pub(super) fn take_artifact_watch(&mut self, instance: EditorInstanceId) -> Option<Vec<Uuid>> {
        self.entries.get_mut(&instance)?.artifact_watch.take()
    }

    pub(super) fn show_dialog(
        &self,
        instance: EditorInstanceId,
        block: Uuid,
        dialog: block_plugin_api::ShellDialog,
    ) -> Vec<Outbound> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        let block = self.local(instance, block.into_bytes());
        vec![Outbound::Editor(
            instance,
            EditorCall::ShowDialog { block, dialog },
        )]
    }

    pub(super) fn publish(
        &mut self,
        open: block_plugin_api::ops::MethodId,
        value: Arc<Vec<u8>>,
    ) -> bool {
        self.published.insert(open, value);
        self.watches_published(open)
    }

    pub(super) fn watches_published(&self, open: block_plugin_api::ops::MethodId) -> bool {
        self.entries
            .iter()
            .any(|(instance, entry)| entry.published.watches(open, self.shell == Some(*instance)))
    }

    pub(super) fn take_calls(
        &mut self,
        accept: &mut dyn FnMut(&block_plugin_api::ops::Called) -> bool,
    ) {
        let mut instances: Vec<_> = self.entries.keys().copied().collect();
        instances.sort_by_key(|instance| instance.0);
        for instance in instances {
            if let Some(entry) = self.entries.get_mut(&instance) {
                entry.published.take_calls(accept);
            }
        }
    }

    pub(super) fn set_shell(&mut self, shell: Option<EditorInstanceId>) -> bool {
        if self.shell == shell {
            return false;
        }
        self.shell = shell;
        true
    }

    pub(super) fn set_focus(&mut self, focus: Focus) -> bool {
        if self.focus == focus {
            return false;
        }
        self.focus = focus;
        true
    }
}

fn through(rect: Rect, laid_view: EditorView, view: EditorView) -> Rect {
    if (laid_view.rect.min, laid_view.scale) == (view.rect.min, view.scale) {
        return rect;
    }
    let scale = ratio(view.scale, laid_view.scale);
    Rect::from_min_size(
        view.rect.min + (rect.min - laid_view.rect.min) * scale,
        rect.size() * scale,
    )
}

fn followed_clip(child: &ChildPlacement, laid_view: EditorView, view: EditorView) -> Rect {
    let clip = child.clip;
    let swept = beui::Rotation::new(child.rect.center(), child.rotation).bounds(child.rect);
    let held = |clipped: f32, own: f32, open: f32| match (clipped - own).abs() > 0.01 {
        true => clipped,
        false => open,
    };
    let held = Rect::from_min_max(
        pos2(
            held(clip.min.x, swept.min.x, f32::MIN),
            held(clip.min.y, swept.min.y, f32::MIN),
        ),
        pos2(
            held(clip.max.x, swept.max.x, f32::MAX),
            held(clip.max.y, swept.max.y, f32::MAX),
        ),
    );
    through(swept, laid_view, view).intersect(held)
}

fn host_rect(rect: Rect, origin: Vec2, scale: Vec2) -> Rect {
    Rect::from_min_size(
        pos2(rect.min.x * scale.x, rect.min.y * scale.y) + origin,
        vec2(rect.width() * scale.x, rect.height() * scale.y),
    )
}

fn ratio(current: f32, published: f32) -> f32 {
    match published > 0.0 && current > 0.0 {
        true => current / published,
        false => 1.0,
    }
}

fn allowed(url: &str, hosts: &[String]) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    hosts.iter().any(|allowed| allowed == host)
}

fn only_moved(sent: Option<AnchorSpace>, now: Option<AnchorSpace>) -> bool {
    match (sent, now) {
        (Some(sent), Some(now)) => {
            sent.bounds == now.bounds && sent.to_host.scale == now.to_host.scale
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;

fn entry_menu_holds(instances: &Instances, instance: EditorInstanceId) -> Option<ViewId> {
    let view = instances.content_view(instance);
    instances
        .entries
        .get(&instance)?
        .menus
        .contains_key(&view)
        .then_some(view)
}
