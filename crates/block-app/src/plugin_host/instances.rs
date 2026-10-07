use be_block::BlockContent as _;
use beui::{ImeArea, Rect, Vec2, pos2, vec2};
use block_plugin_api::ImeArea as PluginImeArea;
use block_plugin_api::{
    ArtifactDescription, AudioCommand, BlockCommand, BlockPick, ChildContent, ChildId, ChildMode,
    ChildPlacement, ChildPlacements, ChildStatus, CreationOutcome, CursorIcon, DataListing,
    EditorInstanceId, EditorMessage, EditorRegion, FetchResult, FilePick, FileSave, FrameReport,
    FrameSpec, HostPanel, HostReply, HostRequest, LinuxMessage, Message, Occluder,
    PerformanceMeasurement, RegenerationOutcome, RegionSize, ScreenId, ScreenLayout, ScreenRequest,
    ScreenSet, Size, ViewChange, WatchedContent, WebViewId,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

use super::{
    BlockPickRequest, ChildCommit, EditorBlock, HostChild, HostChildStatus, InstanceRole,
    MAX_LIVE_CHILDREN,
    audio::AudioPlayer,
    input::{BlockDragEvent, FileDropEvent, InputAdapter, viewport_metrics},
    pieces,
};
use crate::{
    editors::plugin::discovery,
    host, performance,
    platform::{SavedFile, http, save_file},
    plugin_host::web_view::WebViewHost,
};

const REFUSED: &str = "this plugin's manifest does not allow it to reach";

#[derive(Default)]
pub(super) struct Instances {
    entries: HashMap<EditorInstanceId, Instance>,
    focus: Focus,
    connection: Option<Connection>,
    next_screen: u64,
    announced: HashSet<ScreenId>,
    request_id: u64,
    block_types: Option<Arc<block_plugin_api::Catalog>>,
    sent_block_types: bool,
    network: Vec<String>,
    plugin_id: String,
    replies: Replies,
    epoch: u64,
    graph_seen: Option<u64>,
    pasted: HashSet<EditorInstanceId>,
    audio_changes: AudioChanges,
    input_devices: Vec<block_plugin_api::HostInputDevice>,
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

struct Reply {
    epoch: u64,
    instance: EditorInstanceId,
    request_id: u64,
    reply: HostReply,
}

struct Replies {
    sender: host::WakingSender<Reply>,
    receiver: std::sync::mpsc::Receiver<Reply>,
}

impl Default for Replies {
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
    creation_ready: bool,
    created: Option<Result<Uuid, String>>,
    screens: HashMap<EditorRegion, Screen>,
    opened: bool,
    deferred: Vec<Message>,
    opens: Vec<OpenRequest>,
    block_drags: Vec<(Uuid, Uuid)>,
    block_commands: Vec<(Uuid, BlockCommand)>,
    reported_focus: Option<Focus>,
    reported_editable: Option<bool>,
    focus_reports: Vec<Focus>,
    artifact_watch: Option<Vec<Uuid>>,
    reported_artifacts: Vec<block_plugin_api::ArtifactState>,
    history_watch: Vec<Uuid>,
    reported_history: Option<Vec<block_plugin_api::HistoryState>>,
    drag_accepted: bool,
    intrinsic: Option<Vec2>,
    aspect_ratio: Option<f32>,
    text_pastes: Vec<block_plugin_api::InputEvent>,
    audio: Option<AudioPlayer>,
    reported_size: Option<Vec2>,
    block_picks: Vec<BlockPickRequest>,
    pick_answers: Vec<(u64, BlockPick)>,
    child_commits: Vec<ChildCommit>,
    view: Option<EditorView>,
    reported_view: Option<EditorView>,
    view_changes: Vec<ViewChange>,
    bar_actions: Vec<block_plugin_api::BarAction>,
    menu: Vec<block_plugin_api::MenuEntry>,
    child_menu_picks: Vec<(ChildId, String)>,
    presenting: bool,
    reported_presenting: bool,
    windows: Option<Vec<block_plugin_api::HostWindow>>,
    reported_windows: Option<Vec<block_plugin_api::HostWindow>>,
    watches_input_devices: bool,
    reported_input_devices: Option<Vec<block_plugin_api::HostInputDevice>>,
    closed_windows: Vec<block_plugin_api::HostWindowId>,
    grabbed: bool,
    web_views: HashMap<WebViewId, WebViewHost>,
    presence_visible: Option<bool>,
    replacements: HashMap<(Uuid, Uuid), Replacement>,
    next_replacement: u64,
    leaving: bool,
    content: Option<ContentLink>,
    watched: HashMap<Uuid, ContentLink>,
    shown: std::collections::HashSet<(Uuid, Uuid)>,
    block_queries: Vec<block_plugin_api::BlockQuery>,
    sent_blocks: HashMap<block_plugin_api::BlockQuery, Vec<block_plugin_api::BlockInfo>>,
    blocks_seen: Option<u64>,
    version_sent: Option<u64>,
    stale: bool,
}

struct ContentLink {
    content_type: Uuid,
    opened: bool,
    origin: u64,
    sent: Option<u64>,
    peers_sent: Option<u64>,
    described: Option<u64>,
}

impl ContentLink {
    fn new(content_type: Uuid) -> Self {
        Self {
            content_type,
            opened: false,
            origin: crate::be::next_origin(),
            sent: None,
            peers_sent: None,
            described: None,
        }
    }

    fn describe(&mut self, block: Uuid) {
        let Some(revision) = crate::be::content_revision(block) else {
            return;
        };
        if self.described == Some(revision) || !crate::be::access(block).can_edit() {
            return;
        }
        self.described = Some(revision);
        crate::be::describe_implicitly(block, crate::be::describe_block(block).unwrap_or_default());
    }

    fn content_message(&mut self, instance: EditorInstanceId, block: Uuid) -> Option<Message> {
        if crate::be::content_revision(block).is_none() {
            if !std::mem::replace(&mut self.opened, true) {
                crate::be::open(block, self.content_type);
            }
            return None;
        }
        let (revision, update) = crate::be::update_since(block, self.origin, self.sent)?;
        self.sent = Some(revision);
        let block_id = block.into_bytes();
        Some(Message::Editor(match update {
            crate::be::Update::Snapshot {
                content_type,
                bytes,
                applied,
            } => EditorMessage::Content {
                instance,
                block_id,
                content_type: content_type.into_bytes(),
                bytes,
                applied,
            },
            crate::be::Update::Operations(operations) => EditorMessage::ContentOperations {
                instance,
                block_id,
                operations: operations
                    .into_iter()
                    .map(|(operation, mine)| block_plugin_api::ContentOperation { operation, mine })
                    .collect(),
            },
        }))
    }

    fn messages(&mut self, instance: EditorInstanceId, block: Uuid, out: &mut Vec<Message>) {
        out.extend(self.content_message(instance, block));
        if let Some((revision, peers)) = crate::be::presence_since(block, self.peers_sent) {
            self.peers_sent = Some(revision);
            out.push(Message::Editor(EditorMessage::PeerPresence {
                instance,
                block_id: block.into_bytes(),
                peers: peers
                    .into_iter()
                    .map(|peer| block_plugin_api::PeerPresence {
                        client: peer.client,
                        kind: peer.kind.into_bytes(),
                        value: peer.value,
                    })
                    .collect(),
            }));
        }
    }
}

pub(super) type OpenRequest = (Uuid, Uuid, Option<Uuid>);

#[derive(Clone, Default, PartialEq)]
pub(crate) struct Focus {
    pub(crate) block: Option<(Uuid, Uuid)>,
    pub(crate) via: Vec<Uuid>,
}

#[derive(Clone, Copy)]
pub(super) struct Held {
    pub(super) rect: Rect,
    pub(super) drawn: (u32, u32),
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct EditorView {
    pub(crate) rect: Rect,
    pub(crate) scale: f32,
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

impl Instance {
    fn new(role: InstanceRole) -> Self {
        Self {
            role,
            artifact: ArtifactState::default(),
            creation_ready: false,
            created: None,
            screens: HashMap::new(),
            opened: false,
            deferred: Vec::new(),
            opens: Vec::new(),
            block_drags: Vec::new(),
            block_commands: Vec::new(),
            reported_focus: None,
            reported_editable: None,
            focus_reports: Vec::new(),
            artifact_watch: None,
            reported_artifacts: Vec::new(),
            history_watch: Vec::new(),
            reported_history: None,
            drag_accepted: false,
            intrinsic: None,
            aspect_ratio: None,
            text_pastes: Vec::new(),
            audio: None,
            reported_size: None,
            block_picks: Vec::new(),
            pick_answers: Vec::new(),
            child_commits: Vec::new(),
            view: None,
            reported_view: None,
            view_changes: Vec::new(),
            bar_actions: Vec::new(),
            menu: Vec::new(),
            child_menu_picks: Vec::new(),
            presenting: false,
            reported_presenting: false,
            windows: None,
            reported_windows: None,
            watches_input_devices: false,
            reported_input_devices: None,
            closed_windows: Vec::new(),
            grabbed: false,
            web_views: HashMap::new(),
            presence_visible: None,
            replacements: HashMap::new(),
            next_replacement: 0,
            leaving: false,
            watched: HashMap::new(),
            shown: std::collections::HashSet::new(),
            block_queries: Vec::new(),
            sent_blocks: HashMap::new(),
            blocks_seen: None,
            version_sent: None,
            stale: true,
            content: match role {
                InstanceRole::Editor(block) => own_content_type(block).map(ContentLink::new),
                InstanceRole::Creation(..) | InstanceRole::Artifact(..) => None,
            },
        }
    }
}

fn own_content_type(block: EditorBlock) -> Option<Uuid> {
    if crate::be::is_known(block.block_type) {
        return Some(block.block_type);
    }
    (block.view_block == Some(block.id))
        .then_some(<be_block::EditorViewContent as be_block::BlockContent>::CONTENT_TYPE)
}

impl Instance {
    fn blocks_messages(&mut self, instance: EditorInstanceId) -> Vec<Message> {
        let revision = crate::be::graph_revision();
        if self.blocks_seen == Some(revision) || !crate::be::graph_loaded() {
            return Vec::new();
        }
        self.blocks_seen = Some(revision);
        let mut messages = Vec::new();
        for query in &self.block_queries {
            let blocks = super::graph::answer(*query);
            if self.sent_blocks.get(query) == Some(&blocks) {
                continue;
            }
            self.sent_blocks.insert(*query, blocks.clone());
            messages.push(Message::Editor(EditorMessage::Blocks {
                instance,
                query: *query,
                blocks,
            }));
        }
        messages
    }

    fn watch_blocks(&mut self, queries: Vec<block_plugin_api::BlockQuery>) {
        self.sent_blocks.retain(|query, _| queries.contains(query));
        self.block_queries = queries;
        self.blocks_seen = None;
    }

    fn content_messages(&mut self, instance: EditorInstanceId) -> Vec<Message> {
        let mut messages = Vec::new();
        if let (Some(block), Some(link)) = (self.role.block(), self.content.as_mut()) {
            link.messages(instance, block.id, &mut messages);
        }
        for (block, link) in &mut self.watched {
            link.messages(instance, *block, &mut messages);
        }
        if let InstanceRole::Editor(block) = self.role
            && crate::be::is_versioned(block.block_type)
            && let Some((revision, status)) =
                crate::be::version_since(block.id, block.block_type, self.version_sent)
        {
            self.version_sent = Some(revision);
            messages.push(Message::Editor(EditorMessage::VersionStatus {
                instance,
                block_id: block.id.into_bytes(),
                status,
            }));
        }
        messages
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

    fn follows(&self, block: Uuid) -> bool {
        self.role.block().is_some_and(|own| own.id == block)
            || self.watched.contains_key(&block)
            || self.history_watch.contains(&block)
    }
}

impl Instance {
    fn history_message(&mut self, instance: EditorInstanceId) -> Option<Message> {
        if self.history_watch.is_empty() && self.reported_history.is_none() {
            return None;
        }
        let states: Vec<_> = self
            .history_watch
            .iter()
            .map(|block| {
                let history = crate::be::history(*block);
                block_plugin_api::HistoryState {
                    block_id: block.into_bytes(),
                    can_undo: history.can_undo,
                    can_redo: history.can_redo,
                }
            })
            .collect();
        if self.reported_history.as_ref() == Some(&states) {
            return None;
        }
        self.reported_history = Some(states.clone());
        Some(Message::Editor(EditorMessage::HistoryStates {
            instance,
            states,
        }))
    }

    fn name_content(&mut self) {
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
    pub(super) pass: u64,
}

struct Screen {
    input: InputAdapter,
    placement: Option<Placement>,
    request: ScreenRequest,
    last_seen: u64,
    mounted: u32,
    presented: Option<(Rect, Rect)>,
    holding: bool,
    used: Option<Vec2>,
    report: Option<FrameReport>,
    dragging: bool,
    file_dropping: bool,
    cursor: CursorIcon,
    ime: Option<PluginImeArea>,
    children: ChildTable,
    reported_statuses: HashMap<ChildId, ChildStatus>,
    revoked: HashSet<ChildId>,
    frame_revoked: HashSet<ChildId>,
}

impl Screen {
    fn unplace(&mut self) {
        self.placement = None;
        self.request.metrics = viewport_metrics(Vec2::ZERO, Rect::ZERO, 1.0);
    }
}

#[derive(Default, PartialEq)]
struct ChildTable {
    generation: u64,
    size: Vec2,
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
    pub(super) opened: Vec<Message>,
    pub(super) screens: Vec<ScreenRequest>,
}

impl Instances {
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn translate(&self, message: &mut Message, inward: bool) {
        let mut carries = false;
        message.visit_block_ids(&mut |_, _, _| carries = true);
        if !carries {
            return;
        }
        let mut scopes: HashMap<EditorInstanceId, Option<crate::be::Scope>> = HashMap::new();
        crate::be::with_graph(|graph| {
            message.visit_block_ids(&mut |instance, role, id| {
                let scope = *scopes.entry(instance).or_insert_with(|| {
                    let block = self.entries.get(&instance)?.role.block()?.id;
                    graph.scope_of(block)
                });
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

    pub(super) fn gate(&mut self, messages: Vec<Message>) -> Vec<Message> {
        let mut allowed = Vec::with_capacity(messages.len());
        for message in messages {
            match &message {
                Message::Editor(editor) => {
                    let instance = editor.instance();
                    match self.entries.get_mut(&instance) {
                        Some(entry) if entry.opened => allowed.push(message),
                        Some(entry) => entry.deferred.push(message),
                        None if matches!(editor, EditorMessage::Close { .. }) => {
                            allowed.push(message)
                        }
                        None => {}
                    }
                }
                Message::ChildStatuses(statuses) => {
                    let opened: Vec<_> = statuses
                        .iter()
                        .filter(|status| self.is_opened(status.instance))
                        .cloned()
                        .collect();
                    if !opened.is_empty() {
                        allowed.push(Message::ChildStatuses(opened));
                    }
                }
                Message::Input(batch) if !self.announced.contains(&batch.screen) => {}
                _ => allowed.push(message),
            }
        }
        allowed
    }

    fn is_opened(&self, instance: EditorInstanceId) -> bool {
        self.entries
            .get(&instance)
            .is_some_and(|entry| entry.opened)
    }

    pub(super) fn remove(&mut self, instance: EditorInstanceId) -> bool {
        let Some(entry) = self.entries.remove(&instance) else {
            return false;
        };
        for (block, kind) in &entry.shown {
            crate::be::show(*block, *kind, None);
        }
        let own = entry
            .role
            .block()
            .filter(|_| entry.content.is_some())
            .map(|block| block.id);
        for block in own.into_iter().chain(entry.watched.keys().copied()) {
            if !self.holds_content(block) {
                crate::be::close(block);
            }
        }
        entry.opened
    }

    fn editable(&self, block: Uuid) -> bool {
        crate::be::access(block).can_edit()
    }

    fn holds_content(&self, block: Uuid) -> bool {
        self.entries.values().any(|entry| entry.holds(block))
    }

    fn can_view(&self, block: Uuid) -> bool {
        crate::be::access(block).can_view()
    }

    fn watch_content(&mut self, instance: EditorInstanceId, blocks: Vec<WatchedContent>) -> bool {
        let wanted: HashMap<Uuid, Uuid> = blocks
            .into_iter()
            .map(|watched| {
                (
                    Uuid::from_bytes(watched.block_id),
                    Uuid::from_bytes(watched.content_type),
                )
            })
            .filter(|(block, content_type)| {
                crate::be::is_known(*content_type) && self.can_view(*block)
            })
            .collect();
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        let previous = std::mem::take(&mut entry.watched);
        let mut dropped = Vec::new();
        for (block, link) in previous {
            match wanted.get(&block) {
                Some(content_type) if *content_type == link.content_type => {
                    entry.watched.insert(block, link);
                }
                _ => dropped.push(block),
            }
        }
        for (block, content_type) in wanted {
            entry
                .watched
                .entry(block)
                .or_insert_with(|| ContentLink::new(content_type));
        }
        entry.stale = true;
        for block in dropped {
            if !self.holds_content(block) {
                crate::be::close(block);
            }
        }
        true
    }

    fn connect(&mut self, client_id: Uuid) {
        self.connection = Some(Connection { client_id });
    }

    pub(super) fn report(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        client_id: Uuid,
        role: InstanceRole,
        block_types: &Arc<block_plugin_api::Catalog>,
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
        let next_screen = &mut self.next_screen;
        let screen = entry.screens.entry(region).or_insert_with(|| {
            *next_screen += 1;
            Screen {
                input: InputAdapter::default(),
                placement: None,
                request: ScreenRequest {
                    screen: ScreenId(*next_screen),
                    instance,
                    region,
                    metrics: viewport_metrics(size, visible, scale_factor),
                    frame: frame.clone(),
                },
                last_seen: pass,
                mounted: 0,
                presented: None,
                holding: false,
                used: None,
                report: None,
                dragging: false,
                file_dropping: false,
                cursor: CursorIcon::Default,
                ime: None,
                children: ChildTable::default(),
                reported_statuses: HashMap::new(),
                revoked: HashSet::new(),
                frame_revoked: HashSet::new(),
            }
        });
        let metrics = viewport_metrics(size, visible, scale_factor);
        screen.request.metrics = metrics;
        screen.request.frame = frame;
        screen.last_seen = pass;
        screen.request.screen
    }

    pub(super) fn mount(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        client_id: Uuid,
        role: InstanceRole,
        block_types: &Arc<block_plugin_api::Catalog>,
    ) {
        self.report(
            instance,
            region,
            client_id,
            role,
            block_types,
            None,
            Vec2::ZERO,
            Rect::ZERO,
            1.0,
            0,
        );
        if let Some(screen) = self.screen_mut(instance, region) {
            screen.mounted += 1;
        }
    }

    pub(super) fn unmount(&mut self, instance: EditorInstanceId, region: EditorRegion) {
        if let Some(screen) = self.screen_mut(instance, region) {
            screen.mounted = screen.mounted.saturating_sub(1);
            if screen.mounted == 0 {
                screen.unplace();
            }
        }
    }

    pub(super) fn unplace(&mut self, instance: EditorInstanceId, region: EditorRegion) {
        if let Some(screen) = self.screen_mut(instance, region)
            && screen.mounted <= 1
        {
            screen.unplace();
        }
    }

    pub(super) fn has_mounted(&self) -> bool {
        self.entries
            .values()
            .any(|entry| entry.screens.values().any(|screen| screen.mounted > 0))
    }

    pub(super) fn place_mounted(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        frame: Option<FrameSpec>,
        size: Vec2,
        visible: Rect,
        scale_factor: f32,
    ) {
        let Some(screen) = self.screen_mut(instance, region) else {
            return;
        };
        let metrics = viewport_metrics(size, visible, scale_factor);
        if screen.request.metrics != metrics || screen.request.frame != frame {
            screen.request.metrics = metrics;
            screen.request.frame = frame;
        }
    }

    pub(super) fn placement(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<Placement> {
        self.entries.get(&instance)?.screens.get(&region)?.placement
    }

    pub(super) fn screen_id(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<ScreenId> {
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
        region: EditorRegion,
        gesture: beui::BackGesture,
    ) -> Vec<Message> {
        let announced = &self.announced;
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
            .filter(|screen| announced.contains(&screen.request.screen))
        else {
            return Vec::new();
        };
        vec![Message::Input(block_plugin_api::InputBatch {
            screen: screen.request.screen,
            events: vec![screen.input.back(gesture)],
        })]
    }

    pub(super) fn mounted(&self, instance: EditorInstanceId, region: EditorRegion) -> bool {
        self.entries
            .get(&instance)
            .and_then(|entry| entry.screens.get(&region))
            .is_some_and(|screen| screen.mounted > 0)
    }

    fn screen_mut(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<&mut Screen> {
        self.entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
    }

    pub(super) fn forward(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        input: &beui::ForwardedInput,
    ) -> (Vec<Message>, bool) {
        let announced = &self.announced;
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        else {
            return (Vec::new(), false);
        };
        if !announced.contains(&screen.request.screen) {
            return (Vec::new(), false);
        }
        let id = screen.request.screen;
        let pressed = input.events.iter().any(|event| {
            matches!(
                event,
                beui::Event::PointerButton { pressed: true, .. }
                    | beui::Event::Touch {
                        phase: beui::TouchPhase::Start,
                        ..
                    }
            )
        });
        let escaped = input.focused
            && input.events.iter().any(|event| {
                matches!(
                    event,
                    beui::Event::Key {
                        key: beui::Key::Escape,
                        pressed: true,
                        ..
                    }
                )
            });
        let mut messages = screen.input.forward(input, id);
        let dragging = screen.dragging;
        let files = super::input::file_drop(input, screen.file_dropping);
        let drag = super::input::block_drag(input);
        let changed = dragging != drag.as_ref().is_some_and(|drag| !drag.dropped);
        messages.extend(self.drag(instance, region, drag));
        messages.extend(self.file_drop(instance, region, files));
        let revoked =
            (escaped || (pressed && input.hovered)) && self.revoke_active(instance, region);
        (messages, revoked || changed)
    }

    pub(super) fn hold(&mut self, instance: EditorInstanceId, region: EditorRegion) {
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
        region: EditorRegion,
        rect: Option<Rect>,
        clip: Rect,
        drawn: Option<(u32, u32)>,
    ) -> Option<Held> {
        let screen = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))?;
        let requested = (
            screen.request.metrics.pixel_width,
            screen.request.metrics.pixel_height,
        );
        let stale = drawn.filter(|drawn| *drawn != requested);
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

    pub(super) fn set_windows(
        &mut self,
        instance: EditorInstanceId,
        windows: Vec<block_plugin_api::HostWindow>,
    ) -> bool {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        let changed = entry.windows.as_ref() != Some(&windows);
        entry.windows = Some(windows);
        changed
    }

    pub(super) fn set_presenting(&mut self, instance: EditorInstanceId, presenting: bool) -> bool {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        let changed = entry.presenting != presenting;
        entry.presenting = presenting;
        changed
    }

    pub(super) fn resized(&mut self, instance: EditorInstanceId, size: Vec2) -> Vec<Message> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        if entry.reported_size.is_some_and(|reported| {
            (reported.x - size.x).abs() < 0.01 && (reported.y - size.y).abs() < 0.01
        }) {
            return Vec::new();
        }
        entry.reported_size = Some(size);
        vec![Message::Editor(EditorMessage::Resized {
            instance,
            width: size.x,
            height: size.y,
        })]
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
            .map(|entry| entry.menu.clone())
            .unwrap_or_default()
    }

    pub(super) fn menu_pick(&mut self, instance: EditorInstanceId, id: String) -> Vec<Message> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::MenuPick { instance, id })]
    }

    pub(super) fn take_child_menu_picks(
        &mut self,
        instance: EditorInstanceId,
        children: &[ChildId],
    ) -> Vec<(ChildId, String)> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        let (taken, kept) = std::mem::take(&mut entry.child_menu_picks)
            .into_iter()
            .partition(|(child, _)| children.contains(child));
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
        block_types: &Arc<block_plugin_api::Catalog>,
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
        block_types: &Arc<block_plugin_api::Catalog>,
        source_type: Uuid,
        block: EditorBlock,
        data: &[u8],
        resync: bool,
    ) -> Vec<Message> {
        self.connect(client_id);
        if self.block_types.is_none() {
            self.block_types = Some(Arc::clone(block_types));
        }
        let entry = self.entries.entry(instance).or_insert_with(|| {
            let mut entry = Instance::new(InstanceRole::Artifact(source_type, block));
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
        vec![Message::Editor(EditorMessage::ArtifactSettings {
            instance,
            data: data.to_vec(),
        })]
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
    ) -> Vec<Message> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        entry.artifact.outcome = None;
        vec![Message::Editor(EditorMessage::RegenerateArtifact {
            instance,
            data: data.to_vec(),
        })]
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
        self.sent_block_types = false;
        self.announced.clear();
        for entry in self.entries.values_mut() {
            entry.opened = false;
            entry.deferred.clear();
            entry.reported_focus = None;
            entry.reported_editable = None;
            entry.reported_view = None;
            entry.reported_presenting = false;
            entry.reported_windows = None;
            entry.reported_input_devices = None;
            entry.stale = true;
        }
        self.epoch += 1;
    }

    pub(super) fn next_screens(&mut self, pass: u64) -> NextScreens {
        let client = self
            .connection
            .as_ref()
            .map(|connection| connection.client_id)
            .map(|client_id| (client_id, crate::be::identity().unwrap_or_default()));
        let mut instances: Vec<_> = self.entries.keys().copied().collect();
        instances.sort_by_key(|instance| instance.0);
        let focus = self.focus.clone();
        let input_devices = self.input_devices.clone();
        let graph = crate::be::graph_revision();
        let graph_moved = self.graph_seen.replace(graph) != Some(graph);
        let mut opened = Vec::new();
        let mut screens = Vec::new();
        if !self.sent_block_types
            && let Some(block_types) = &self.block_types
        {
            self.sent_block_types = true;
            opened.push(Message::BlockTypes(block_types.as_ref().clone()));
        }
        for instance in instances {
            let Some(entry) = self.entries.get_mut(&instance) else {
                continue;
            };
            let mut regions: Vec<_> = entry
                .screens
                .values()
                .filter(|screen| {
                    (screen.mounted > 0 || screen.last_seen >= pass)
                        && screen.request.metrics.pixel_width > 0
                        && screen.request.metrics.pixel_height > 0
                })
                .map(|screen| screen.request.clone())
                .collect();
            if regions.is_empty() && matches!(entry.role, InstanceRole::Editor(_)) {
                continue;
            }
            regions.sort_by_key(|request| request.screen.0);
            let Some((client_id, (account, workspace))) = client else {
                continue;
            };
            let client_id = client_id.into_bytes();
            if !entry.opened {
                entry.opened = true;
                let account_id = account.into_bytes();
                let workspace_id = workspace.into_bytes();
                opened.push(match entry.role {
                    InstanceRole::Editor(block) => Message::Editor(EditorMessage::Open {
                        instance,
                        block_id: block.id.into_bytes(),
                        block_type: block.block_type.into_bytes(),
                        view_block: block.view_block.map(Uuid::into_bytes),
                        account_id,
                        workspace_id,
                        client_id,
                        editable: {
                            let editable = crate::be::access(block.id).can_edit();
                            entry.reported_editable = Some(editable);
                            editable
                        },
                    }),
                    InstanceRole::Creation(editor, template) => {
                        Message::Editor(EditorMessage::OpenCreation {
                            instance,
                            block_type: editor.into_bytes(),
                            template: template.to_owned(),
                            account_id,
                            workspace_id,
                            client_id,
                        })
                    }
                    InstanceRole::Artifact(source_type, block) => {
                        Message::Editor(EditorMessage::OpenArtifact {
                            instance,
                            source_type: source_type.into_bytes(),
                            block_id: block.id.into_bytes(),
                            block_type: block.block_type.into_bytes(),
                            account_id,
                            workspace_id,
                            client_id,
                            data: entry.artifact.data.clone(),
                        })
                    }
                });
            }
            opened.append(&mut entry.deferred);
            let stale = std::mem::take(&mut entry.stale);
            if let InstanceRole::Editor(block) = entry.role
                && (stale || graph_moved)
            {
                let editable = crate::be::access(block.id).can_edit();
                if entry.reported_editable != Some(editable) {
                    entry.reported_editable = Some(editable);
                    opened.push(Message::Editor(EditorMessage::EditabilityChanged {
                        instance,
                        editable,
                    }));
                }
            }
            opened.extend(entry.blocks_messages(instance));
            if stale {
                opened.extend(entry.content_messages(instance));
                if let Some(message) = entry.history_message(instance) {
                    opened.push(message);
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
                opened.push(Message::Editor(EditorMessage::FocusChanged {
                    instance,
                    block_id,
                    block_type,
                    via: focus.via.iter().map(|id| id.into_bytes()).collect(),
                }));
            }
            if entry.presenting != entry.reported_presenting {
                entry.reported_presenting = entry.presenting;
                opened.push(Message::Editor(EditorMessage::PresentingChanged {
                    instance,
                    presenting: entry.presenting,
                }));
            }
            if entry.watches_input_devices
                && entry.reported_input_devices.as_ref() != Some(&input_devices)
            {
                entry.reported_input_devices = Some(input_devices.clone());
                opened.push(Message::Editor(EditorMessage::Linux {
                    instance,
                    message: LinuxMessage::InputDevices(input_devices.clone()),
                }));
            }
            if entry.windows.is_some() && entry.windows != entry.reported_windows {
                entry.reported_windows = entry.windows.clone();
                opened.push(Message::Editor(EditorMessage::Linux {
                    instance,
                    message: LinuxMessage::Windows(entry.windows.clone().unwrap_or_default()),
                }));
            }
            if entry.view != entry.reported_view {
                entry.reported_view = entry.view;
                if let Some(view) = entry.view {
                    opened.push(Message::Editor(EditorMessage::ViewChanged {
                        instance,
                        x: view.rect.min.x,
                        y: view.rect.min.y,
                        width: view.rect.width(),
                        height: view.rect.height(),
                        scale: view.scale,
                    }));
                }
            }
            screens.extend(regions);
        }
        NextScreens { opened, screens }
    }

    pub(super) fn set_region_sizes(&mut self, sizes: Vec<RegionSize>) -> bool {
        let mut changed = false;
        for size in sizes {
            for entry in self.entries.values_mut() {
                if let Some(screen) = entry
                    .screens
                    .values_mut()
                    .find(|screen| screen.request.screen == size.screen)
                {
                    let used = vec2(size.logical_width, size.logical_height);
                    changed |= screen.used != Some(used);
                    screen.used = Some(used);
                }
            }
        }
        changed
    }

    pub(super) fn drag(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        event: Option<BlockDragEvent>,
    ) -> Vec<Message> {
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
                vec![Message::Editor(EditorMessage::DragOver {
                    instance,
                    region,
                    x: event.position.x,
                    y: event.position.y,
                    block_id: event.block_id.into_bytes(),
                    block_type: event.block_type.into_bytes(),
                    dropped: event.dropped,
                })]
            }
            None if screen.dragging => {
                screen.dragging = false;
                entry.drag_accepted = false;
                vec![Message::Editor(EditorMessage::DragLeft { instance })]
            }
            None => Vec::new(),
        }
    }

    pub(super) fn file_drop(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        event: Option<FileDropEvent>,
    ) -> Vec<Message> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        let Some(screen) = entry.screens.get_mut(&region) else {
            return Vec::new();
        };
        match event {
            Some(event) => {
                screen.file_dropping = !event.dropped;
                vec![Message::Editor(EditorMessage::FileDrop {
                    instance,
                    region,
                    x: event.position.x,
                    y: event.position.y,
                    files: event.files,
                    dropped: event.dropped,
                })]
            }
            None if screen.file_dropping => {
                screen.file_dropping = false;
                vec![Message::Editor(EditorMessage::FileDropLeft { instance })]
            }
            None => Vec::new(),
        }
    }

    pub(super) fn dragging(&self, instance: EditorInstanceId, region: EditorRegion) -> bool {
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

    pub(super) fn set_children(&mut self, placements: ChildPlacements) -> (Vec<Message>, bool) {
        let ChildPlacements {
            instance,
            region,
            generation,
            size,
            children,
            occluders,
        } = placements;
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        else {
            return (Vec::new(), false);
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
            generation,
            size: vec2(size.width, size.height),
            children,
            occluders,
        };
        let changed = screen.children != table;
        screen.children = table;
        (Vec::new(), changed)
    }

    pub(super) fn host_children(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
        rect: Rect,
        clip: Rect,
    ) -> (Vec<HostChild>, Holes) {
        let Some(screen) = self
            .entries
            .get(&instance)
            .and_then(|entry| entry.screens.get(&region))
        else {
            return (Vec::new(), Holes::default());
        };
        let table = &screen.children;
        let origin = rect.min.to_vec2();
        let stretch = vec2(
            ratio(rect.width(), table.size.x),
            ratio(rect.height(), table.size.y),
        );
        let mut children = Vec::new();
        let mut holes = Holes::default();
        let mut live = 0;
        if let Some(report) = &screen.report {
            let painted: Vec<Rect> = report
                .painted
                .iter()
                .map(|painted| host_rect(*painted, origin, stretch))
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
        for (index, child) in table.children.iter().enumerate() {
            let content = match &child.content {
                ChildContent::Block {
                    block_id,
                    block_type,
                    view_block,
                } => super::HostContent::Block {
                    block_id: Uuid::from_bytes(*block_id),
                    block_type: Uuid::from_bytes(*block_type),
                    view_block: view_block.map(Uuid::from_bytes),
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
                ChildContent::WebView(_) => continue,
            };
            if child.rect.is_empty() {
                continue;
            }
            let requested = match (region, child.mode) {
                (EditorRegion::Preview, _) => ChildMode::Preview,
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
            let child_rect = host_rect(child.rect, origin, stretch);
            let child_clip = host_rect(child.clip, origin, stretch).intersect(clip);
            let occluders: Vec<Rect> = table
                .occluders
                .iter()
                .filter(|occluder| occluder.after as usize > index)
                .map(|occluder| host_rect(occluder.rect, origin, stretch))
                .collect();
            if matches!(mode, ChildMode::Active | ChildMode::Live) {
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
                occluders,
                clip: child_clip,
                layer: child.layer,
                mode,
                intrinsic: child.intrinsic.map(|size| vec2(size.width, size.height)),
                rotation: child.rotation,
                opacity: child.opacity,
            });
        }
        (children, holes)
    }

    pub(super) fn pressed_at(&mut self, position: beui::Pos2) -> bool {
        let outside: Vec<(EditorInstanceId, EditorRegion)> = self
            .entries
            .iter()
            .flat_map(|(instance, entry)| {
                entry.screens.iter().filter_map(move |(region, screen)| {
                    let placement = screen.placement?;
                    let away = screen.mounted > 0
                        && !placement.rect.intersect(placement.clip).contains(position);
                    away.then_some((*instance, *region))
                })
            })
            .collect();
        let mut revoked = false;
        for (instance, region) in outside {
            revoked |= self.revoke_active(instance, region);
        }
        revoked
    }

    pub(super) fn revoke_active(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> bool {
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

    pub(super) fn frame_child(&self, instance: EditorInstanceId) -> Option<Uuid> {
        let screen = self
            .entries
            .get(&instance)?
            .screens
            .get(&EditorRegion::Frame)?;
        screen
            .children
            .children
            .iter()
            .filter(|child| {
                matches!(child.mode, ChildMode::Active | ChildMode::Live)
                    && !child.own_frame
                    && !screen.frame_revoked.contains(&child.child)
                    && !screen.revoked.contains(&child.child)
                    && !child.rect.is_empty()
            })
            .find_map(|child| child.content.block_id())
            .map(Uuid::from_bytes)
    }

    pub(super) fn revoke_frame_child(&mut self, instance: EditorInstanceId) {
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&EditorRegion::Frame))
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
        region: EditorRegion,
        statuses: Vec<HostChildStatus>,
    ) -> Vec<Message> {
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
                region,
                child: status.child,
                available: status.available,
                intrinsic: status.intrinsic.map(|size| Size {
                    width: size.x,
                    height: size.y,
                }),
                aspect_ratio: status.aspect_ratio,
                hovered: status.hovered,
                active: status.active,
                interaction: status.interaction,
                capabilities: status.capabilities,
                resize: status.resize,
                error: status.error,
                menu: status.menu,
                creation: status.creation,
                settings: status.settings,
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
            false => vec![Message::ChildStatuses(changed)],
        }
    }

    pub(super) fn take_block_pick(
        &mut self,
        instance: EditorInstanceId,
    ) -> Option<BlockPickRequest> {
        let entry = self.entries.get_mut(&instance)?;
        match entry.block_picks.is_empty() {
            true => None,
            false => Some(entry.block_picks.remove(0)),
        }
    }

    pub(super) fn take_pick_answers(
        &mut self,
        instance: EditorInstanceId,
    ) -> Vec<(u64, BlockPick)> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.pick_answers))
            .unwrap_or_default()
    }

    pub(super) fn take_child_commits(&mut self, instance: EditorInstanceId) -> Vec<ChildCommit> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.child_commits))
            .unwrap_or_default()
    }

    pub(super) fn request_pick(
        &self,
        instance: EditorInstanceId,
        pick: u64,
        filter: block_plugin_api::BlockFilter,
        parent: block_plugin_api::BlockLocation,
    ) -> Vec<Message> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::PickRequested {
            instance,
            pick,
            filter,
            parent,
        })]
    }

    pub(super) fn block_picked(
        &self,
        instance: EditorInstanceId,
        request_id: u64,
        pick: BlockPick,
    ) -> Vec<Message> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::Replied {
            instance,
            request_id,
            reply: HostReply::BlockPicked(pick),
        })]
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
                            region: screen.request.region,
                            logical: vec2(metrics.logical_width, metrics.logical_height),
                            pixels: [metrics.pixel_width, metrics.pixel_height],
                            scale_factor: metrics.scale_factor,
                            used: screen.used,
                            placement,
                            drawn: screen.mounted > 0 || screen.last_seen >= pass,
                            children: screen.children.children.len(),
                            child_generation: screen.children.generation,
                        }
                    })
                    .collect();
                screens.sort_by_key(|screen| screen.screen.0);
                super::InstanceStatus {
                    instance: *instance,
                    block: entry.role.block().map(|block| block.id),
                    role: match entry.role {
                        InstanceRole::Editor(_) => "editor",
                        InstanceRole::Creation(..) => "creation",
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
                                    ArtifactDescription::Described { summary, .. } => {
                                        summary.clone()
                                    }
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

    pub(super) fn creation_ready(&self, instance: EditorInstanceId) -> bool {
        self.entries
            .get(&instance)
            .is_some_and(|entry| entry.creation_ready)
    }

    pub(super) fn commit_creation(&self, instance: EditorInstanceId) -> Vec<Message> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::CommitCreation { instance })]
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

    pub(super) fn region_size(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<Vec2> {
        self.entries.get(&instance)?.screens.get(&region)?.used
    }

    pub(super) fn set_frame_reports(&mut self, reports: Vec<FrameReport>) -> bool {
        let mut changed = false;
        for report in reports {
            for entry in self.entries.values_mut() {
                if let Some(screen) = entry
                    .screens
                    .values_mut()
                    .find(|screen| screen.request.screen == report.screen)
                {
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
        region: EditorRegion,
    ) -> Option<&FrameReport> {
        self.entries
            .get(&instance)?
            .screens
            .get(&region)?
            .report
            .as_ref()
    }

    pub(super) fn drive_web_views(&mut self, pass: u64) -> Vec<Message> {
        let mut messages = Vec::new();
        for (instance, entry) in &mut self.entries {
            if entry.web_views.is_empty() {
                continue;
            }
            let mut rects = HashMap::new();
            for screen in entry.screens.values() {
                let Some(placement) = screen.placement else {
                    continue;
                };
                let live =
                    screen.mounted > 0 || (placement.pass == pass && screen.last_seen == pass);
                if !live {
                    continue;
                }
                let origin = placement.rect.min.to_vec2();
                let stretch = vec2(
                    ratio(placement.rect.width(), screen.request.metrics.logical_width),
                    ratio(
                        placement.rect.height(),
                        screen.request.metrics.logical_height,
                    ),
                );
                for child in &screen.children.children {
                    let ChildContent::WebView(web_view) = child.content else {
                        continue;
                    };
                    if child.rect.is_empty() {
                        continue;
                    }
                    let clip = host_rect(child.clip, origin, stretch).intersect(placement.clip);
                    rects.insert(
                        web_view,
                        host_rect(child.rect, origin, stretch).intersect(clip),
                    );
                }
            }
            for (web_view, view) in &mut entry.web_views {
                let mut events = Vec::new();
                view.drive(rects.get(web_view).copied(), &mut events);
                for event in events {
                    messages.push(Message::Editor(EditorMessage::WebViewEvent {
                        instance: *instance,
                        web_view: *web_view,
                        event,
                    }));
                }
            }
        }
        messages
    }

    pub(super) fn set_presence_visible(
        &mut self,
        instance: EditorInstanceId,
        visible: bool,
    ) -> Vec<Message> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        if entry.presence_visible == Some(visible) {
            return Vec::new();
        }
        entry.presence_visible = Some(visible);
        vec![Message::Editor(EditorMessage::Presence {
            instance,
            visible,
        })]
    }

    pub(super) fn child_view_changes(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        changes: Vec<(ChildId, ViewChange)>,
    ) -> Vec<Message> {
        changes
            .into_iter()
            .map(|(child, change)| {
                Message::Editor(EditorMessage::ChildView {
                    instance,
                    region,
                    child,
                    change,
                })
            })
            .collect()
    }

    pub(super) fn child_bar_actions(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        actions: Vec<(ChildId, block_plugin_api::BarAction)>,
    ) -> Vec<Message> {
        actions
            .into_iter()
            .map(|(child, action)| {
                Message::Editor(EditorMessage::ChildBar {
                    instance,
                    region,
                    child,
                    action,
                })
            })
            .collect()
    }

    pub(super) fn replace_child(
        &mut self,
        instance: EditorInstanceId,
        old: Uuid,
        new: Uuid,
    ) -> (Vec<Message>, Option<bool>) {
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
                    vec![Message::Editor(EditorMessage::ReplaceChild {
                        instance,
                        request_id,
                        old: old.into_bytes(),
                        new: new.into_bytes(),
                    })],
                    None,
                )
            }
        }
    }

    pub(super) fn grabbing(&self) -> bool {
        self.entries.values().any(|entry| entry.grabbed)
    }

    pub(super) fn ime(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
        rect: Rect,
    ) -> Option<ImeArea> {
        let screen = self.entries.get(&instance)?.screens.get(&region)?;
        let area = screen.ime.as_ref()?;
        let origin = rect.min.to_vec2();
        let stretch = vec2(
            ratio(rect.width(), screen.request.metrics.logical_width),
            ratio(rect.height(), screen.request.metrics.logical_height),
        );
        Some(ImeArea {
            rect: host_rect(area.rect, origin, stretch),
            cursor: host_rect(area.cursor, origin, stretch),
            text: area.text.as_ref().map(beui_plugin_input::beui_ime_text),
            keyboard: area.keyboard,
        })
    }

    pub(super) fn cursor(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<beui::CursorIcon> {
        let screen = self.entries.get(&instance)?.screens.get(&region)?;
        Some(match screen.cursor {
            CursorIcon::Default => beui::CursorIcon::Default,
            CursorIcon::None => beui::CursorIcon::None,
            CursorIcon::Pointer => beui::CursorIcon::PointingHand,
            CursorIcon::Text => beui::CursorIcon::Text,
            CursorIcon::Crosshair => beui::CursorIcon::Crosshair,
            CursorIcon::Grab => beui::CursorIcon::Grab,
            CursorIcon::Grabbing => beui::CursorIcon::Grabbing,
            CursorIcon::Move => beui::CursorIcon::Move,
            CursorIcon::NotAllowed => beui::CursorIcon::NotAllowed,
            CursorIcon::Wait => beui::CursorIcon::Wait,
            CursorIcon::Progress => beui::CursorIcon::Progress,
            CursorIcon::Help => beui::CursorIcon::Help,
            CursorIcon::ResizeHorizontal => beui::CursorIcon::ResizeHorizontal,
            CursorIcon::ResizeVertical => beui::CursorIcon::ResizeVertical,
            CursorIcon::ResizeNeSw => beui::CursorIcon::ResizeNeSw,
            CursorIcon::ResizeNwSe => beui::CursorIcon::ResizeNwSe,
        })
    }

    pub(super) fn screen_set(&mut self, screens: Vec<ScreenRequest>) -> Message {
        self.announced = screens.iter().map(|screen| screen.screen).collect();
        self.request_id += 1;
        Message::Screens(ScreenSet {
            request_id: self.request_id,
            screens,
        })
    }

    pub(super) fn place(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
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

    pub(super) fn pending(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        while let Ok(reply) = self.replies.receiver.try_recv() {
            if reply.epoch == self.epoch && self.entries.contains_key(&reply.instance) {
                messages.push(Message::Editor(EditorMessage::Replied {
                    instance: reply.instance,
                    request_id: reply.request_id,
                    reply: reply.reply,
                }));
            }
        }
        let mut pasted: Vec<_> = std::mem::take(&mut self.pasted).into_iter().collect();
        pasted.sort_by_key(|instance| instance.0);
        for instance in pasted {
            let Some(entry) = self.entries.get_mut(&instance) else {
                continue;
            };
            let texts = std::mem::take(&mut entry.text_pastes);
            if !texts.is_empty()
                && let Some(screen) = entry
                    .screens
                    .values()
                    .find(|screen| screen.input.focused())
                    .or_else(|| entry.screens.get(&EditorRegion::Frame))
                    .map(|screen| screen.request.screen)
            {
                messages.push(Message::Input(block_plugin_api::InputBatch {
                    screen,
                    events: texts,
                }));
            }
        }
        let mut played: Vec<_> = self.audio_changes.receiver.try_iter().collect();
        played.sort_by_key(|instance| instance.0);
        played.dedup();
        for instance in played {
            if let Some(player) = self
                .entries
                .get(&instance)
                .and_then(|entry| entry.audio.as_ref())
            {
                messages.push(Message::Editor(EditorMessage::AudioStatus {
                    instance,
                    status: player.status(),
                }));
            }
        }
        messages
    }

    fn request(
        &mut self,
        instance: EditorInstanceId,
        request_id: u64,
        request: HostRequest,
    ) -> bool {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        let sender = self.replies.sender.clone();
        let epoch = self.epoch;
        let reply = move |reply: HostReply| {
            let _ = sender.send(Reply {
                epoch,
                instance,
                request_id,
                reply,
            });
        };
        match request {
            HostRequest::PickFile(filter) => {
                let filter = beui::FileFilter {
                    name: filter.name,
                    extensions: filter.extensions,
                    mime_types: filter.mime_types,
                };
                host::pick_file(filter, move |picked| {
                    reply(HostReply::FilePicked(match picked {
                        Ok(Some(file)) => FilePick::Chosen {
                            name: file.name,
                            data: file.data,
                        },
                        Ok(None) => FilePick::Cancelled,
                        Err(error) => FilePick::Failed(error),
                    }));
                });
            }
            HostRequest::SaveFile(file) => {
                let file = SavedFile {
                    name: file.name,
                    mime_type: file.mime_type,
                    data: file.data,
                };
                save_file(file, move |saved| {
                    reply(HostReply::FileSaved(match saved {
                        Ok(true) => FileSave::Saved,
                        Ok(false) => FileSave::Cancelled,
                        Err(error) => FileSave::Failed(error),
                    }));
                });
            }
            HostRequest::PasteImage => reply(HostReply::ImagePasted(
                super::clipboard::read_clipboard_image(),
            )),
            HostRequest::Fetch(url) => {
                let fetched = move |body: Result<Vec<u8>, String>| {
                    reply(HostReply::Fetched(fetch_result(body)));
                };
                match allowed(&url, &self.network) {
                    true => http::fetch(url, Vec::new(), fetched),
                    false => fetched(Err(format!("{REFUSED} {url}"))),
                }
            }
            HostRequest::ListData => discovery::data_listing(&self.plugin_id, move |index| {
                reply(HostReply::DataListed(match index {
                    Ok(index) => match serde_json::from_slice(&index) {
                        Ok(files) => DataListing::Files(files),
                        Err(error) => DataListing::Failed(format!("the data index is {error}")),
                    },
                    Err(error) => DataListing::Failed(error),
                }));
            }),
            HostRequest::ReadData(path) => discovery::data(&self.plugin_id, &path, move |body| {
                reply(HostReply::DataRead(fetch_result(body)));
            }),
            HostRequest::ListAccess(block) => {
                crate::be::list_access(Uuid::from_bytes(block), move |listed| {
                    reply(HostReply::AccessListed(super::graph::grants_of(listed)));
                });
            }
            HostRequest::PickBlock(filter) => {
                entry
                    .block_picks
                    .push(BlockPickRequest { request_id, filter });
            }
        }
        true
    }

    pub(super) fn editor_message(&mut self, message: EditorMessage) -> bool {
        match message {
            EditorMessage::PickAnswered {
                instance,
                pick,
                answer,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.pick_answers.push((pick, answer));
                true
            }
            EditorMessage::CloseWindow { instance, window } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.closed_windows.push(window);
                true
            }
            EditorMessage::Linux { instance, message } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                match message {
                    LinuxMessage::WatchInputDevices => entry.watches_input_devices = true,
                    LinuxMessage::Windows(_) | LinuxMessage::InputDevices(_) => return false,
                }
                true
            }
            EditorMessage::SetAccess {
                block_id,
                account,
                access,
                ..
            } => {
                crate::be::set_access(
                    Uuid::from_bytes(block_id),
                    Uuid::from_bytes(account),
                    super::graph::access_of(access),
                );
                true
            }
            EditorMessage::CommitChild {
                instance,
                child,
                parent,
                name,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.child_commits.push(ChildCommit {
                    child,
                    parent: super::graph::parent_of(parent),
                    name,
                });
                true
            }
            EditorMessage::OpenBlock {
                instance,
                block_id,
                block_type,
                via,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.opens.push((
                    Uuid::from_bytes(block_id),
                    Uuid::from_bytes(block_type),
                    via.map(Uuid::from_bytes),
                ));
                true
            }
            EditorMessage::Focused {
                instance,
                block_id,
                block_type,
                via,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.focus_reports.push(Focus {
                    block: block_id
                        .map(|block_id| (Uuid::from_bytes(block_id), Uuid::from_bytes(block_type))),
                    via: via.into_iter().map(Uuid::from_bytes).collect(),
                });
                true
            }
            EditorMessage::WatchHistory { instance, blocks } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.history_watch = blocks.into_iter().map(Uuid::from_bytes).collect();
                entry.stale = true;
                true
            }
            EditorMessage::WatchArtifacts { instance, blocks } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.artifact_watch = Some(blocks.into_iter().map(Uuid::from_bytes).collect());
                true
            }
            EditorMessage::DragBlock {
                instance,
                block_id,
                block_type,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry
                    .block_drags
                    .push((Uuid::from_bytes(block_id), Uuid::from_bytes(block_type)));
                true
            }
            EditorMessage::BlockCommand {
                instance,
                block_id,
                command,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry
                    .block_commands
                    .push((Uuid::from_bytes(block_id), command));
                true
            }
            EditorMessage::Request {
                instance,
                request_id,
                request,
            } => self.request(instance, request_id, request),
            EditorMessage::Operate {
                instance,
                block_id,
                operation,
            } => {
                let block = Uuid::from_bytes(block_id);
                if !self.editable(block) {
                    return false;
                }
                let Some(link) = self
                    .entries
                    .get_mut(&instance)
                    .and_then(|entry| entry.link_mut(block))
                else {
                    return false;
                };
                crate::be::operate_from(block, link.origin, operation);
                true
            }
            EditorMessage::WatchContent { instance, blocks } => {
                self.watch_content(instance, blocks)
            }
            EditorMessage::ResendContent { instance, block_id } => {
                let Some(link) = self
                    .entries
                    .get_mut(&instance)
                    .and_then(|entry| entry.link_mut(Uuid::from_bytes(block_id)))
                else {
                    return false;
                };
                link.sent = None;
                if let Some(entry) = self.entries.get_mut(&instance) {
                    entry.stale = true;
                }
                true
            }
            EditorMessage::VersionControl {
                instance,
                block_id,
                command,
            } => {
                let block = Uuid::from_bytes(block_id);
                let own = self
                    .entries
                    .get(&instance)
                    .and_then(|entry| entry.role.block())
                    .is_some_and(|own| own.id == block);
                if !own || !self.editable(block) {
                    return false;
                }
                if let block_plugin_api::VersionCommand::Adopt { block_id } = &command
                    && !self.editable(Uuid::from_bytes(*block_id))
                {
                    return false;
                }
                crate::be::version(block, command);
                true
            }
            EditorMessage::ReplaceContent {
                block_id,
                content_type,
                bytes,
                ..
            } => {
                let block = Uuid::from_bytes(block_id);
                let content_type = Uuid::from_bytes(content_type);
                if crate::be::is_known(content_type) && self.editable(block) {
                    crate::be::replace(block, content_type, bytes);
                }
                false
            }
            EditorMessage::WatchBlocks { instance, queries } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.watch_blocks(queries);
                true
            }
            EditorMessage::CreateBlock {
                block_id,
                content_type,
                parent,
                name,
                artifact,
                content,
                ..
            } => {
                let parent = super::graph::parent_of(parent);
                if parent.block().is_some_and(|parent| !self.editable(parent)) {
                    return false;
                }
                let block = Uuid::from_bytes(block_id);
                if crate::be::node(block).is_some() {
                    return false;
                }
                let metadata = be_block::BlockMetadata {
                    named_by_hand: name.is_some(),
                    name,
                    artifact: artifact.map(|artifact| be_block::ArtifactSource {
                        source_type: Uuid::from_bytes(artifact.source_type),
                        data: artifact.data,
                    }),
                    local_id: None,
                    derived: be_block::DerivedMetadata::default(),
                };
                crate::be::create(
                    block,
                    Uuid::from_bytes(content_type),
                    parent,
                    metadata,
                    content.map(|content| content.into_vec()),
                );
                true
            }
            EditorMessage::SetParent {
                block_id, parent, ..
            } => {
                let block = Uuid::from_bytes(block_id);
                let parent = super::graph::parent_of(parent);
                if !self.editable(block)
                    || parent.block().is_some_and(|parent| !self.editable(parent))
                {
                    return false;
                }
                crate::be::set_parent(block, parent);
                true
            }
            EditorMessage::SetName { block_id, name, .. } => {
                let block = Uuid::from_bytes(block_id);
                if !self.editable(block) {
                    return false;
                }
                crate::be::set_name(block, name);
                true
            }
            EditorMessage::ShowPresence {
                instance,
                block_id,
                kind,
                value,
            } => {
                let block = Uuid::from_bytes(block_id);
                let kind = Uuid::from_bytes(kind);
                if !self.can_view(block) {
                    return false;
                }
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                if !entry.holds(block) {
                    return false;
                }
                match value.is_some() {
                    true => entry.shown.insert((block, kind)),
                    false => entry.shown.remove(&(block, kind)),
                };
                crate::be::show(block, kind, value.map(|value| value.into_vec()));
                false
            }
            EditorMessage::SeedContent {
                instance,
                block_id,
                content_type,
                bytes,
            } => {
                let block = Uuid::from_bytes(block_id);
                let content_type = Uuid::from_bytes(content_type);
                let holds = self
                    .entries
                    .get(&instance)
                    .is_some_and(|entry| entry.holds(block));
                if holds && crate::be::is_known(content_type) {
                    crate::be::seed(block, content_type, bytes);
                }
                false
            }
            EditorMessage::DragAccepted { instance, accepted } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let changed = entry.drag_accepted != accepted;
                entry.drag_accepted = accepted;
                changed
            }
            EditorMessage::PlayAudio {
                instance,
                block_id,
                command,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let changes = self.audio_changes.sender.clone();
                let player = entry.audio.get_or_insert_with(|| {
                    AudioPlayer::new(move || {
                        let _ = changes.send(instance);
                    })
                });
                match command {
                    AudioCommand::Reset => player.reset(),
                    AudioCommand::Toggle => {
                        let audio = crate::be::content(Uuid::from_bytes(block_id))
                            .and_then(|held| be_block::AudioContent::decode(&held.bytes).ok());
                        if let Some(audio) = audio {
                            player.toggle(&audio);
                        }
                    }
                }
                true
            }
            EditorMessage::WebViewCommand {
                instance,
                web_view,
                command,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry
                    .web_views
                    .entry(web_view)
                    .or_default()
                    .command(command);
                true
            }
            EditorMessage::GrabCursor { instance, grabbed } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.grabbed = grabbed;
                true
            }
            EditorMessage::CreationReady { instance, ready } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let changed = entry.creation_ready != ready;
                entry.creation_ready = ready;
                changed
            }
            EditorMessage::CreationBlock { instance, outcome } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.created = Some(match outcome {
                    CreationOutcome::Created(block_id) => Ok(Uuid::from_bytes(block_id)),
                    CreationOutcome::Failed(error) => Err(error),
                });
                true
            }
            EditorMessage::Ime {
                instance,
                region,
                area,
            } => {
                let Some(screen) = self
                    .entries
                    .get_mut(&instance)
                    .and_then(|entry| entry.screens.get_mut(&region))
                else {
                    return false;
                };
                let changed = screen.ime != area;
                screen.ime = area;
                changed
            }
            EditorMessage::CopyText { instance, text } => {
                if !self.entries.contains_key(&instance) {
                    return false;
                }
                host::copy_text(text);
                false
            }
            EditorMessage::PasteText { instance } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                if let Some(text) = super::clipboard::read_clipboard_text() {
                    entry
                        .text_pastes
                        .extend(block_plugin_api::paste_events(&text));
                }
                self.pasted.insert(instance);
                true
            }
            EditorMessage::ChildReplaced {
                instance,
                request_id,
                replaced,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
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
                true
            }
            EditorMessage::Cursor {
                instance,
                region,
                cursor,
            } => {
                let Some(screen) = self
                    .entries
                    .get_mut(&instance)
                    .and_then(|entry| entry.screens.get_mut(&region))
                else {
                    return false;
                };
                let changed = screen.cursor != cursor;
                screen.cursor = cursor;
                changed
            }
            EditorMessage::ArtifactDescribed {
                instance,
                description,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.artifact.description = Some(description);
                true
            }
            EditorMessage::ArtifactEdited { instance, data } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let changed = entry.artifact.draft.as_ref() != Some(&data);
                entry.artifact.draft = Some(data);
                changed
            }
            EditorMessage::ArtifactRegenerated { instance, outcome } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.artifact.outcome = Some(match outcome {
                    RegenerationOutcome::Done => Ok(()),
                    RegenerationOutcome::Failed(error) => Err(error),
                });
                true
            }
            EditorMessage::Present {
                instance,
                presenting,
            } => self.set_presenting(instance, presenting),
            EditorMessage::LeaveFrame { instance } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.leaving = true;
                true
            }
            EditorMessage::BarAction { instance, action } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.bar_actions.push(action);
                true
            }
            EditorMessage::Menu { instance, entries } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.menu = entries;
                true
            }
            EditorMessage::ChildMenuPick {
                instance,
                child,
                id,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.child_menu_picks.push((child, id));
                true
            }
            EditorMessage::ChangeView { instance, change } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.view_changes.push(change);
                true
            }
            EditorMessage::AspectRatio { instance, ratio } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let changed = entry.aspect_ratio != ratio;
                entry.aspect_ratio = ratio;
                changed
            }
            EditorMessage::IntrinsicSize { instance, size } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                let intrinsic = size.map(|size| vec2(size.width, size.height));
                let changed = entry.intrinsic != intrinsic;
                entry.intrinsic = intrinsic;
                changed
            }
            EditorMessage::Performance {
                instance,
                group,
                measurements,
            } if self.entries.contains_key(&instance) => {
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
            _ => false,
        }
    }

    pub(super) fn take_open(&mut self, instance: EditorInstanceId) -> Option<OpenRequest> {
        let entry = self.entries.get_mut(&instance)?;
        if entry.opens.is_empty() {
            None
        } else {
            Some(entry.opens.remove(0))
        }
    }

    pub(super) fn take_block_drag(&mut self, instance: EditorInstanceId) -> Option<(Uuid, Uuid)> {
        let entry = self.entries.get_mut(&instance)?;
        if entry.block_drags.is_empty() {
            None
        } else {
            Some(entry.block_drags.remove(0))
        }
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

    pub(super) fn take_closed_windows(
        &mut self,
        instance: EditorInstanceId,
    ) -> Vec<block_plugin_api::HostWindowId> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.closed_windows))
            .unwrap_or_default()
    }

    pub(super) fn take_artifact_watch(&mut self, instance: EditorInstanceId) -> Option<Vec<Uuid>> {
        self.entries.get_mut(&instance)?.artifact_watch.take()
    }

    pub(super) fn show_dialog(
        &mut self,
        instance: EditorInstanceId,
        block: Uuid,
        dialog: block_plugin_api::ShellDialog,
    ) -> Vec<Message> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::ShowDialog {
            instance,
            block_id: block.into_bytes(),
            dialog,
        })]
    }

    pub(super) fn show_panel(
        &mut self,
        instance: EditorInstanceId,
        panel: HostPanel,
    ) -> Vec<Message> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::ShowPanel {
            instance,
            panel,
        })]
    }

    pub(super) fn show_block(
        &mut self,
        instance: EditorInstanceId,
        block_id: Uuid,
        block_type: Uuid,
        via: Option<Uuid>,
    ) -> Vec<Message> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::ShowBlock {
            instance,
            block_id: block_id.into_bytes(),
            block_type: block_type.into_bytes(),
            via: via.map(Uuid::into_bytes),
        })]
    }

    pub(super) fn set_artifact_states(
        &mut self,
        instance: EditorInstanceId,
        states: Vec<block_plugin_api::ArtifactState>,
    ) -> Vec<Message> {
        let Some(entry) = self.entries.get_mut(&instance) else {
            return Vec::new();
        };
        if entry.reported_artifacts == states {
            return Vec::new();
        }
        entry.reported_artifacts = states.clone();
        vec![Message::Editor(EditorMessage::ArtifactStates {
            instance,
            states,
        })]
    }

    pub(super) fn set_input_devices(
        &mut self,
        devices: Vec<block_plugin_api::HostInputDevice>,
    ) -> bool {
        self.input_devices = devices;
        self.entries
            .values()
            .any(|entry| entry.watches_input_devices)
    }

    pub(super) fn set_focus(&mut self, focus: Focus) -> bool {
        if self.focus == focus {
            return false;
        }
        self.focus = focus;
        true
    }
}

fn host_rect(rect: block_plugin_api::ChildRect, origin: Vec2, stretch: Vec2) -> Rect {
    Rect::from_min_size(
        pos2(rect.x * stretch.x, rect.y * stretch.y) + origin,
        vec2(rect.width * stretch.x, rect.height * stretch.y),
    )
}

fn ratio(current: f32, published: f32) -> f32 {
    match published > 0.0 && current > 0.0 {
        true => current / published,
        false => 1.0,
    }
}

fn fetch_result(body: Result<Vec<u8>, String>) -> FetchResult {
    match body {
        Ok(body) => FetchResult::Body(body),
        Err(error) => FetchResult::Failed(error),
    }
}

fn allowed(url: &str, hosts: &[String]) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    hosts.iter().any(|allowed| allowed == host)
}

#[cfg(test)]
mod tests;
