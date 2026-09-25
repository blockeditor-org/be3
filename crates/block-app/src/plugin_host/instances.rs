use be_block::BlockContent as _;
use beui::{ImeArea, Rect, Vec2, pos2, vec2};
use block_plugin_api::ImeArea as PluginImeArea;
use block_plugin_api::{
    ArtifactDescription, AudioCommand, AudioStatus, BlockCommand, BlockPick, BlockTypeDescriptor,
    ChildId, ChildMode, ChildPlacement, ChildPlacements, ChildStatus, ClipboardImage,
    CreationOutcome, CursorIcon, EditorInstanceId, EditorMessage, EditorRegion, FetchResult,
    FilePick, FrameReport, FrameSpec, HostReply, HostRequest, Message, Occluder, PaneId,
    PaneLayout, PaneTree, PerformanceMeasurement, RegenerationOutcome, RegionSize, ScreenId, ScreenLayout, ScreenRequest,
    ScreenSet, Size, ViewChange, WatchedContent,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;

use super::{
    BlockPickRequest, EditorBlock, HostChild, HostChildStatus, InstanceRole, MAX_LIVE_CHILDREN,
    audio::AudioPlayer,
    input::{BlockDragEvent, FileDropEvent, InputAdapter, viewport_metrics},
    pieces,
};
use crate::{
    host::{self, Target},
    performance,
    platform::{FileFilter, FilePicker, http::Fetch},
    plugin_host::web_view::WebViewHost,
};

const FETCH_POLL_INTERVAL: Duration = Duration::from_millis(100);
const REFUSED: &str = "this plugin's manifest does not allow it to reach";

#[derive(Default)]
pub(super) struct Instances {
    entries: HashMap<EditorInstanceId, Instance>,
    focus: Focus,
    connection: Option<Connection>,
    next_screen: u64,
    announced: HashSet<ScreenId>,
    request_id: u64,
    block_types: Option<Arc<Vec<BlockTypeDescriptor>>>,
    sent_block_types: bool,
    network: Vec<String>,
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
    pending: Vec<Pending>,
    text_pastes: Vec<String>,
    audio: Option<AudioPlayer>,
    reported_audio: AudioStatus,
    reported_size: Option<Vec2>,
    block_picks: Vec<BlockPickRequest>,
    view: Option<EditorView>,
    reported_view: Option<EditorView>,
    view_changes: Vec<ViewChange>,
    presenting: bool,
    reported_presenting: bool,
    grabbed: bool,
    web_view: Option<WebViewHost>,
    web_view_rect: Option<(EditorRegion, block_plugin_api::ChildRect)>,
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
    panes: Option<PaneLayout>,
    shown_panes: Vec<PaneId>,
}

struct ContentLink {
    content_type: Uuid,
    opened: bool,
    origin: u64,
    sent: Option<u64>,
    peers_sent: Option<u64>,
    named: Option<u64>,
}

impl ContentLink {
    fn new(content_type: Uuid) -> Self {
        Self {
            content_type,
            opened: false,
            origin: crate::be::next_origin(),
            sent: None,
            peers_sent: None,
            named: None,
        }
    }

    fn name(&mut self, block: Uuid) {
        let Some(content) = crate::be::content(block) else {
            return;
        };
        if self.named == Some(content.revision) || !crate::be::access(block).can_edit() {
            return;
        }
        self.named = Some(content.revision);
        crate::be::name_implicitly(block, crate::be::name_of(&content));
    }

    fn content_message(&mut self, instance: EditorInstanceId, block: Uuid) -> Option<Message> {
        if crate::be::content(block).is_none() {
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
    pub(super) clip: Rect,
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
            pending: Vec::new(),
            text_pastes: Vec::new(),
            audio: None,
            reported_audio: AudioStatus::default(),
            reported_size: None,
            block_picks: Vec::new(),
            view: None,
            reported_view: None,
            view_changes: Vec::new(),
            presenting: false,
            reported_presenting: false,
            grabbed: false,
            web_view: None,
            web_view_rect: None,
            presence_visible: None,
            replacements: HashMap::new(),
            next_replacement: 0,
            leaving: false,
            watched: HashMap::new(),
            shown: std::collections::HashSet::new(),
            block_queries: Vec::new(),
            sent_blocks: HashMap::new(),
            blocks_seen: None,
            panes: None,
            shown_panes: Vec::new(),
            content: match role {
                InstanceRole::Editor(block) => crate::be::is_known(block.block_type)
                    .then(|| ContentLink::new(block.block_type)),
                InstanceRole::Creation | InstanceRole::Artifact(_) => None,
            },
        }
    }
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
        let mut messages = self.blocks_messages(instance);
        if let (Some(block), Some(link)) = (self.role.block(), self.content.as_mut()) {
            link.messages(instance, block.id, &mut messages);
        }
        for (block, link) in &mut self.watched {
            link.messages(instance, *block, &mut messages);
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
            link.name(block.id);
        }
        for (block, link) in &mut self.watched {
            link.name(*block);
        }
    }
}

struct Pending {
    request_id: u64,
    work: Work,
}

enum Work {
    Pick(FilePicker),
    Fetch(Fetch),
    Paste(ClipboardImage),
}

impl Work {
    fn poll(&mut self) -> Option<HostReply> {
        match self {
            Self::Pick(picker) => Some(HostReply::FilePicked(match picker.poll() {
                Some(Ok(file)) => FilePick::Chosen {
                    name: file.name,
                    data: file.data,
                },
                Some(Err(error)) => FilePick::Failed(error),
                None if picker.is_open() => return None,
                None => FilePick::Cancelled,
            })),
            Self::Fetch(fetch) => match fetch.poll() {
                Some(Ok(body)) => Some(HostReply::Fetched(FetchResult::Body(body))),
                Some(Err(error)) => Some(HostReply::Fetched(FetchResult::Failed(error))),
                None => {
                    host::request_repaint_after(FETCH_POLL_INTERVAL);
                    None
                }
            },
            Self::Paste(image) => Some(HostReply::ImagePasted(std::mem::replace(
                image,
                ClipboardImage::Empty,
            ))),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Placement {
    pub(super) target: Target,
    pub(super) rect: Rect,
    pub(super) clip: Rect,
    pub(super) pass: u64,
}

struct Screen {
    input: InputAdapter,
    placement: Option<Placement>,
    request: ScreenRequest,
    last_seen: u64,
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

#[derive(Clone, Default)]
pub(super) struct FrameOverlay {
    pub(super) owner: Option<EditorInstanceId>,
    pub(super) rects: Vec<Rect>,
}

impl FrameOverlay {
    pub(super) fn covering(&self, instance: EditorInstanceId) -> &[Rect] {
        match self.owner == Some(instance) {
            true => &[],
            false => &self.rects,
        }
    }
}

#[derive(Default)]
pub(super) struct Holes {
    holes: Vec<Hole>,
}

impl Holes {
    pub(super) fn cover(&mut self, rects: &[Rect]) {
        for rect in rects {
            self.holes.push(Hole {
                rect: *rect,
                occluders: Vec::new(),
            });
        }
    }

    pub(super) fn contains(&self, position: beui::Pos2) -> bool {
        self.holes.iter().any(|hole| {
            hole.rect.contains(position)
                && !hole
                    .occluders
                    .iter()
                    .any(|occluder| occluder.contains(position))
        })
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
        block_types: &Arc<Vec<BlockTypeDescriptor>>,
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
        screen.request.metrics = viewport_metrics(size, visible, scale_factor);
        screen.request.frame = frame;
        screen.last_seen = pass;
        screen.request.screen
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
            && let (Some(drawn), Some((rect, clip))) = (stale, screen.presented)
        {
            return Some(Held { rect, clip, drawn });
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
        block_types: &Arc<Vec<BlockTypeDescriptor>>,
    ) -> bool {
        self.connect(client_id);
        if self.block_types.is_none() {
            self.block_types = Some(Arc::clone(block_types));
        }
        self.entries
            .entry(instance)
            .or_insert_with(|| Instance::new(InstanceRole::Creation))
            .opened
    }

    pub(super) fn report_artifact(
        &mut self,
        instance: EditorInstanceId,
        client_id: Uuid,
        block_types: &Arc<Vec<BlockTypeDescriptor>>,
        block: EditorBlock,
        data: &[u8],
        resync: bool,
    ) -> Vec<Message> {
        self.connect(client_id);
        if self.block_types.is_none() {
            self.block_types = Some(Arc::clone(block_types));
        }
        let entry = self.entries.entry(instance).or_insert_with(|| {
            let mut entry = Instance::new(InstanceRole::Artifact(block));
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
            entry.pending.clear();
        }
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
                    screen.last_seen >= pass
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
                        account_id,
                        workspace_id,
                        client_id,
                        editable: {
                            let editable = crate::be::access(block.id).can_edit();
                            entry.reported_editable = Some(editable);
                            editable
                        },
                    }),
                    InstanceRole::Creation => Message::Editor(EditorMessage::OpenCreation {
                        instance,
                        account_id,
                        workspace_id,
                        client_id,
                    }),
                    InstanceRole::Artifact(block) => Message::Editor(EditorMessage::OpenArtifact {
                        instance,
                        block_id: block.id.into_bytes(),
                        block_type: block.block_type.into_bytes(),
                        account_id,
                        workspace_id,
                        client_id,
                        data: entry.artifact.data.clone(),
                    }),
                });
            }
            opened.append(&mut entry.deferred);
            if let InstanceRole::Editor(block) = entry.role {
                let editable = crate::be::access(block.id).can_edit();
                if entry.reported_editable != Some(editable) {
                    entry.reported_editable = Some(editable);
                    opened.push(Message::Editor(EditorMessage::EditabilityChanged {
                        instance,
                        editable,
                    }));
                }
            }
            opened.extend(entry.content_messages(instance));
            entry.name_content();
            if let Some(message) = entry.history_message(instance) {
                opened.push(message);
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
            size: vec2(
                screen.request.metrics.logical_width,
                screen.request.metrics.logical_height,
            ),
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
            if matches!(mode, ChildMode::Active | ChildMode::Live) {
                let interactive = child_rect.intersect(child_clip);
                if interactive.is_positive() {
                    holes.holes.push(Hole {
                        rect: interactive,
                        occluders: table
                            .occluders
                            .iter()
                            .filter(|occluder| occluder.after as usize > index)
                            .map(|occluder| host_rect(occluder.rect, origin, stretch))
                            .collect(),
                    });
                }
            }
            children.push(HostChild {
                child: child.child,
                frame_owner: matches!(mode, ChildMode::Active | ChildMode::Live)
                    && !screen.frame_revoked.contains(&child.child),
                own_frame: child.own_frame,
                top_bar: child.top_bar,
                block_id: Uuid::from_bytes(child.block_id),
                block_type: Uuid::from_bytes(child.block_type),
                rect: child_rect,
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

    pub(super) fn revoke_active(&mut self, instance: EditorInstanceId, region: EditorRegion) {
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        else {
            return;
        };
        for child in &screen.children.children {
            if child.mode == ChildMode::Active {
                screen.revoked.insert(child.child);
            }
        }
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
            .find(|child| {
                matches!(child.mode, ChildMode::Active | ChildMode::Live)
                    && !child.own_frame
                    && !screen.frame_revoked.contains(&child.child)
                    && !child.rect.is_empty()
            })
            .map(|child| Uuid::from_bytes(child.block_id))
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
        let live: Vec<ChildId> = statuses.iter().map(|status| status.child).collect();
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
                                [placement.x, placement.y, placement.width, placement.height]
                            });
                        super::ScreenStatus {
                            screen: screen.request.screen,
                            region: screen.request.region,
                            logical: vec2(metrics.logical_width, metrics.logical_height),
                            pixels: [metrics.pixel_width, metrics.pixel_height],
                            scale_factor: metrics.scale_factor,
                            used: screen.used,
                            placement,
                            drawn: screen.last_seen >= pass,
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
                        InstanceRole::Creation => "creation",
                        InstanceRole::Artifact(_) => "artifact",
                    },
                    opened: entry.opened,
                    aspect_ratio: entry.aspect_ratio,
                    intrinsic: entry.intrinsic,
                    view: entry.view.map(|view| view.rect),
                    artifact: matches!(entry.role, InstanceRole::Artifact(_)).then(|| {
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

    pub(super) fn frame_report(&self, instance: EditorInstanceId) -> Option<&FrameReport> {
        self.entries
            .get(&instance)?
            .screens
            .get(&EditorRegion::Frame)?
            .report
            .as_ref()
    }

    pub(super) fn drive_web_views(&mut self, pass: u64) -> Vec<Message> {
        let mut messages = Vec::new();
        for (instance, entry) in &mut self.entries {
            if entry.web_view.is_none() {
                continue;
            }
            let rect = entry.web_view_rect.and_then(|(region, rect)| {
                let screen = entry.screens.get(&region)?;
                let placement = screen.placement?;
                let live = placement.pass == pass && screen.last_seen == pass;
                let origin = placement.rect.min.to_vec2();
                let stretch = vec2(
                    ratio(placement.rect.width(), screen.request.metrics.logical_width),
                    ratio(
                        placement.rect.height(),
                        screen.request.metrics.logical_height,
                    ),
                );
                live.then(|| host_rect(rect, origin, stretch).intersect(placement.clip))
            });
            let mut events = Vec::new();
            let view = entry.web_view.as_mut().expect("the web view is present");
            view.drive(rect, &mut events);
            for event in events {
                messages.push(Message::Editor(EditorMessage::WebViewEvent {
                    instance: *instance,
                    event,
                }));
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
        let area = screen.ime?;
        let origin = rect.min.to_vec2();
        let stretch = vec2(
            ratio(rect.width(), screen.request.metrics.logical_width),
            ratio(rect.height(), screen.request.metrics.logical_height),
        );
        Some(ImeArea {
            rect: host_rect(area.rect, origin, stretch),
            cursor: host_rect(area.cursor, origin, stretch),
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

    pub(super) fn frame_input(&mut self, pass: u64, overlay: &FrameOverlay) -> Vec<Message> {
        let announced = &self.announced;
        let mut placed: Vec<_> = self
            .entries
            .iter()
            .flat_map(|(instance, entry)| {
                entry.screens.iter().filter_map(move |(region, screen)| {
                    let placement = screen.placement?;
                    let live = placement.pass == pass
                        && screen.last_seen == pass
                        && announced.contains(&screen.request.screen);
                    live.then_some((*instance, *region, screen.request.screen, placement))
                })
            })
            .collect();
        placed.sort_by_key(|(instance, _, screen, _)| (instance.0, screen.0));
        let cycle = if host::consume_key(beui::Modifiers::SHIFT, beui::Key::F6) {
            Some(true)
        } else if host::consume_key(beui::Modifiers::NONE, beui::Key::F6) {
            Some(false)
        } else {
            None
        };
        if let Some(backward) = cycle {
            cycle_focus(
                placed.iter().map(|(_, _, _, placement)| placement),
                backward,
            );
        }
        let mut messages = Vec::new();
        for (instance, region, screen, placement) in placed {
            let (_, mut holes) =
                self.host_children(instance, region, placement.rect, placement.clip);
            holes.cover(overlay.covering(instance));
            let focused = host::focused(placement.target);
            let hovered = host::hovered(placement.target);
            messages.extend(self.input(instance, region, |input| {
                input.update(
                    placement.target,
                    placement.rect,
                    hovered,
                    focused,
                    screen,
                    &holes,
                )
            }));
            let over_hole = host::pointer().is_some_and(|position| holes.contains(position));
            let dismissed = host::key_pressed(beui::Key::Escape)
                || host::input(|input| input.primary_pressed && !over_hole);
            if dismissed {
                self.revoke_active(instance, region);
            }
        }
        messages
    }

    fn input(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        update: impl FnOnce(&mut InputAdapter) -> Vec<Message>,
    ) -> Vec<Message> {
        self.entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
            .map(|screen| update(&mut screen.input))
            .unwrap_or_default()
    }

    pub(super) fn pending(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        let mut instances: Vec<_> = self.entries.keys().copied().collect();
        instances.sort_by_key(|instance| instance.0);
        for instance in instances {
            let entry = self.entries.get_mut(&instance).unwrap();
            let mut waiting = std::mem::take(&mut entry.pending);
            waiting.retain_mut(|pending| {
                let Some(reply) = pending.work.poll() else {
                    return true;
                };
                messages.push(Message::Editor(EditorMessage::Replied {
                    instance,
                    request_id: pending.request_id,
                    reply,
                }));
                false
            });
            let entry = self.entries.get_mut(&instance).unwrap();
            entry.pending = waiting;
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
                    events: texts
                        .into_iter()
                        .map(block_plugin_api::InputEvent::Paste)
                        .collect(),
                }));
            }
            let entry = self.entries.get_mut(&instance).unwrap();
            if let Some(player) = &entry.audio {
                let status = player.status();
                if status.playing {
                    host::request_repaint();
                }
                if status != entry.reported_audio {
                    entry.reported_audio.clone_from(&status);
                    messages.push(Message::Editor(EditorMessage::AudioStatus {
                        instance,
                        status,
                    }));
                }
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
        let fetch = match &request {
            HostRequest::Fetch(url) => Some(match allowed(url, &self.network) {
                true => Fetch::get(url.clone(), Vec::new()),
                false => Fetch::refused(format!("{REFUSED} {url}")),
            }),
            _ => None,
        };
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        let work = match request {
            HostRequest::PickFile(filter) => {
                let mut picker = FilePicker::default();
                picker.open(&host_filter(filter));
                Work::Pick(picker)
            }
            HostRequest::PasteImage => Work::Paste(super::clipboard::read_clipboard_image()),
            HostRequest::Fetch(_) => match fetch {
                Some(fetch) => Work::Fetch(fetch),
                None => return false,
            },
            HostRequest::PickBlock(filter) => {
                entry.block_picks.push(BlockPickRequest {
                    request_id,
                    block_types: filter
                        .block_types
                        .into_iter()
                        .map(Uuid::from_bytes)
                        .collect(),
                    excluded: filter.excluded.into_iter().map(Uuid::from_bytes).collect(),
                    templates: filter.templates,
                });
                return true;
            }
        };
        entry.pending.push(Pending { request_id, work });
        true
    }

    pub(super) fn editor_message(&mut self, message: EditorMessage) -> bool {
        match message {
            EditorMessage::Panes { instance, layout } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.panes = Some(layout);
                true
            }
            EditorMessage::ShowPane { instance, pane } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.shown_panes.push(pane);
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
                block_id,
                content_type,
                bytes,
                ..
            } => {
                let block = Uuid::from_bytes(block_id);
                let content_type = Uuid::from_bytes(content_type);
                if crate::be::is_known(content_type) {
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
                let player = entry.audio.get_or_insert_with(AudioPlayer::new);
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
            EditorMessage::WebView {
                instance,
                region,
                rect,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.web_view_rect = rect.map(|rect| (region, rect));
                true
            }
            EditorMessage::WebViewCommand { instance, command } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.web_view.get_or_insert_default().command(command);
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
                entry
                    .text_pastes
                    .extend(super::clipboard::read_clipboard_text());
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

    pub(super) fn panes(&self, instance: EditorInstanceId) -> Option<PaneLayout> {
        self.entries.get(&instance)?.panes.clone()
    }

    pub(super) fn take_shown_panes(&mut self, instance: EditorInstanceId) -> Vec<PaneId> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.shown_panes))
            .unwrap_or_default()
    }

    pub(super) fn arrange_panes(
        &mut self,
        instance: EditorInstanceId,
        arrangement: u64,
        tree: PaneTree,
        detached: Vec<PaneId>,
        focused: Option<PaneId>,
    ) -> Vec<Message> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::PanesArranged {
            instance,
            arrangement,
            tree,
            detached,
            focused,
        })]
    }

    pub(super) fn close_pane(&mut self, instance: EditorInstanceId, pane: PaneId) -> Vec<Message> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::ClosePane { instance, pane })]
    }

    pub(super) fn take_artifact_watch(&mut self, instance: EditorInstanceId) -> Option<Vec<Uuid>> {
        self.entries.get_mut(&instance)?.artifact_watch.take()
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

fn host_filter(filter: block_plugin_api::FileFilter) -> FileFilter {
    FileFilter {
        name: filter.name,
        default_file_name: filter.default_file_name,
        extensions: filter.extensions,
        mime_types: filter.mime_types,
    }
}

fn allowed(url: &str, hosts: &[String]) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    hosts.iter().any(|allowed| allowed == host)
}

fn cycle_focus<'a>(placements: impl Iterator<Item = &'a Placement>, backward: bool) {
    let mut order: Vec<_> = placements
        .filter(|placement| placement.rect.width() > 0.0 && placement.rect.height() > 0.0)
        .map(|placement| (placement.rect.min, placement.target))
        .collect();
    if order.is_empty() {
        return;
    }
    order.sort_by(|(a, _), (b, _)| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    let focused = host::focus();
    let current = order
        .iter()
        .position(|(_, target)| Some(*target) == focused);
    let count = order.len();
    let next = match (current, backward) {
        (Some(index), false) => (index + 1) % count,
        (Some(index), true) => (index + count - 1) % count,
        (None, false) => 0,
        (None, true) => count - 1,
    };
    host::request_focus(order[next].1);
}

#[cfg(test)]
mod tests;
