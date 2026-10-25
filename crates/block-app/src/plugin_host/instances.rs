use beui::accessibility::Embedded;
use beui::{ImeArea, Rect, Vec2, pos2, vec2};
use block_plugin_api::{
    ArtifactDescription, AudioCommand, BlockCommand, BlockImport, BlockPick, ChildContent, ChildId,
    ChildMode, ChildPlacement, ChildPlacements, ChildStatus, CreationOutcome, DataListing,
    EditorInstanceId, EditorMessage, EditorRegion, FetchResult, FileSave, FrameReport, FrameSpec,
    HostPanel, HostReply, HostRequest, Message, Monitor, Occluder, PerformanceMeasurement,
    RegenerationOutcome, RegionSize, ScreenId, ScreenLayout, ScreenRequest, ScreenSet,
    ScreenWindow, Size, Stopped, ViewChange, WatchedContent, WebViewCommand, WebViewEvent,
    WebViewId,
};
use block_plugin_api::{DescribedAction, WindowRequests};
use block_type_guest::editor::{FromEditor, Gesture};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

use super::{
    BlockPickRequest, ChildCommit, EditorBlock, HostChild, HostChildStatus, InstanceRole,
    MAX_LIVE_CHILDREN,
    audio::AudioPlayer,
    heads::{HeadReads, Heads},
    host_values::{Published, Watched},
    input::{BlockDrag, BlockDragEvent, FileDropEvent, InputAdapter, viewport_metrics},
    pieces,
    sent::Sent,
};
use crate::{editors::plugin::discovery, host, performance, platform::http};

thread_local! {
    static PRESENCE_WANTED: std::cell::RefCell<std::collections::HashSet<EditorInstanceId>> =
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
    sent_block_types: Option<u64>,
    network: Vec<String>,
    plugin_id: String,
    replies: Replies,
    epoch: u64,
    graph_seen: Option<u64>,
    audio_changes: AudioChanges,
    head_reads: HeadReads,
    published: Published,
    shell: Option<EditorInstanceId>,
    waiting: HashMap<EditorInstanceId, Vec<Message>>,
    gone: HashSet<EditorInstanceId>,
    serial: u64,
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
    block_drags: Vec<BlockDrag>,
    block_commands: Vec<(Uuid, BlockCommand)>,
    reported_focus: Option<Focus>,
    reported_editable: Option<bool>,
    focus_reports: Vec<Focus>,
    artifact_watch: Option<Vec<Uuid>>,
    reported_artifacts: Vec<block_plugin_api::ArtifactState>,
    history_watch: Vec<Uuid>,
    heads: Heads,
    revision_previews: HashSet<Uuid>,
    reported_history: Option<Vec<block_plugin_api::HistoryState>>,
    drag_accepted: bool,
    intrinsic: Option<Vec2>,
    aspect_ratio: Option<f32>,
    audio: Option<AudioPlayer>,
    reported_size: Option<Vec2>,
    block_picks: Vec<BlockPickRequest>,
    pick_answers: Vec<(u64, BlockPick)>,
    child_commits: Vec<ChildCommit>,
    view: Option<EditorView>,
    reported_view: Option<EditorView>,
    views: Sent<EditorView>,
    view_changes: Vec<ViewChange>,
    bar_actions: Vec<block_plugin_api::BarAction>,
    menu: Vec<block_plugin_api::MenuEntry>,
    child_menu_picks: Vec<(ChildId, String)>,
    presenting: bool,
    presented: bool,
    reported_presenting: bool,
    host_values: Watched,
    unshown_web_views: Vec<WebViewId>,
    presence_visible: Option<bool>,
    replacements: HashMap<(Uuid, Uuid), Replacement>,
    next_replacement: u64,
    leaving: bool,
    content: Option<ContentLink>,
    pending_frames: Vec<(Uuid, Vec<u8>)>,
    early_frames: Vec<(Uuid, Vec<u8>)>,
    watched: HashMap<Uuid, ContentLink>,
    shown: std::collections::HashSet<(Uuid, Uuid)>,
    block_queries: Vec<block_plugin_api::BlockQuery>,
    sent_blocks:
        HashMap<block_plugin_api::BlockQuery, Result<Vec<block_plugin_api::BlockInfo>, String>>,
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
        if !crate::be::access(block).can_edit() {
            return;
        }
        let Some(derived) = crate::be::describe_block(block) else {
            return;
        };
        if self.described.as_ref() == Some(&derived) {
            return;
        }
        self.described = Some(derived.clone());
        crate::be::describe_implicitly(block, derived);
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
            crate::be::from_editor(block, instance.0, &block_type_api::encode_frame(&goodbye));
        }
        crate::be::detach(block, instance.0);
    }
}

const EARLY_FRAMES: usize = 64;

pub(super) type OpenRequest = crate::editors::BlockOpen;

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
            heads: Heads::default(),
            revision_previews: HashSet::new(),
            reported_history: None,
            drag_accepted: false,
            intrinsic: None,
            aspect_ratio: None,
            audio: None,
            reported_size: None,
            block_picks: Vec::new(),
            pick_answers: Vec::new(),
            child_commits: Vec::new(),
            view: None,
            reported_view: None,
            views: Sent::default(),
            view_changes: Vec::new(),
            bar_actions: Vec::new(),
            menu: Vec::new(),
            child_menu_picks: Vec::new(),
            presenting: false,
            presented: false,
            reported_presenting: false,
            host_values: Watched::default(),
            unshown_web_views: Vec::new(),
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
            pending_frames: Vec::new(),
            early_frames: Vec::new(),
            content: match role {
                InstanceRole::Editor(_, block) => own_content_type(block).map(ContentLink::new),
                InstanceRole::Creation(..) | InstanceRole::Artifact(..) => None,
            },
        }
    }
}

fn own_content_type(block: EditorBlock) -> Option<Uuid> {
    crate::be::provided(block.block_type).then_some(block.block_type)
}

impl Instance {
    fn blocks_messages(&mut self, instance: EditorInstanceId) -> Vec<Message> {
        let revision = crate::be::graph_revision();
        if self.blocks_seen == Some(revision) {
            return Vec::new();
        }
        self.blocks_seen = Some(revision);
        let mut messages = Vec::new();
        for query in &self.block_queries {
            let graph_query = super::graph::query_of(*query);
            let answer = match crate::be::ready(graph_query) {
                true => Ok(super::graph::answer(*query)),
                false => match crate::be::failure(graph_query) {
                    Some(error) => Err(error),
                    None => continue,
                },
            };
            if self.sent_blocks.get(query) == Some(&answer) {
                continue;
            }
            self.sent_blocks.insert(*query, answer.clone());
            messages.push(Message::Editor(match answer {
                Ok(blocks) => EditorMessage::Blocks {
                    instance,
                    query: *query,
                    blocks,
                },
                Err(error) => EditorMessage::BlocksFailed {
                    instance,
                    query: *query,
                    error,
                },
            }));
        }
        messages
    }

    fn wants(&self) -> Vec<crate::be::Query> {
        let mut wants: Vec<crate::be::Query> = self
            .block_queries
            .iter()
            .map(|query| super::graph::query_of(*query))
            .collect();
        if let Some(block) = self.role.block() {
            wants.push(crate::be::Query::Parents(block.id));
        }
        wants.extend(
            self.watched
                .keys()
                .map(|block| crate::be::Query::Block(*block)),
        );
        wants
    }

    fn watch_blocks(&mut self, queries: Vec<block_plugin_api::BlockQuery>) {
        self.sent_blocks.retain(|query, _| queries.contains(query));
        self.block_queries = queries;
        self.blocks_seen = None;
    }

    fn content_messages(&mut self, instance: EditorInstanceId) -> Vec<Message> {
        let mut messages = Vec::new();
        if let InstanceRole::Editor(_, block) = self.role
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
            let editable = crate::be::access(block).can_edit();
            if attached != editable {
                link.attached = Some(editable);
                crate::be::attach(block, link.content_type, instance.0, editable);
            }
        }
    }

    fn relay_frames(&mut self, instance: EditorInstanceId, out: &mut Vec<Message>) {
        let mut frames = std::mem::take(&mut self.pending_frames);
        frames.extend(crate::be::take_editor_frames(instance.0));
        let own = self.role.block().map(|block| block.id);
        for (block, link) in self
            .content
            .iter_mut()
            .filter_map(|link| own.map(|own| (own, link)))
            .chain(self.watched.iter_mut().map(|(block, link)| (*block, link)))
        {
            if link.awaiting_preview
                && let Some(frame) = crate::be::snapshot_frame(block)
            {
                link.awaiting_preview = false;
                frames.push((block, frame));
            }
        }
        for (block, frame) in frames {
            if let Some(link) = self.link_mut(block) {
                link.delivered = true;
            } else {
                continue;
            }
            out.push(Message::Editor(EditorMessage::Replica {
                instance,
                block_id: block.into_bytes(),
                frame,
            }));
        }
    }

    fn follows(&self, block: Uuid) -> bool {
        self.role.block().is_some_and(|own| {
            own.id == block || crate::be::revisions::source(own.id) == Some(block)
        }) || self.watched.contains_key(&block)
            || self.history_watch.contains(&block)
            || self.heads.follows(block)
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
                    head: crate::be::node(*block)
                        .and_then(|node| node.head)
                        .map(|head| *head.hash().as_bytes()),
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
        if self
            .role
            .block()
            .is_some_and(|block| crate::be::revisions::source(block.id).is_some())
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
    input: InputAdapter,
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
    serial: u64,
}

impl Screen {
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

    fn unplace(&mut self) {
        self.placement = None;
        self.request.metrics = viewport_metrics(Vec2::ZERO, Rect::ZERO, 1.0, Vec::new());
    }
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
    pub(super) opened: Vec<Message>,
    pub(super) screens: Vec<ScreenRequest>,
}

impl Instances {
    pub(super) fn translate(&self, message: &mut Message, inward: bool) {
        let mut carries = false;
        message.visit_block_ids(&mut |_, _, _| carries = true);
        if !carries {
            return;
        }
        let mut scopes: HashMap<EditorInstanceId, Option<crate::be::Scope>> = HashMap::new();
        message.visit_block_ids(&mut |instance, _, _| {
            scopes.entry(instance).or_insert_with(|| {
                let block = self.entries.get(&instance)?.role.block()?.id;
                let source = crate::be::revisions::source(block).unwrap_or(block);
                crate::be::with_graph(|graph| graph.scope_of(source)).flatten()
            });
        });
        crate::be::with_graph(|graph| {
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

    pub(super) fn hold_unreported(&mut self, messages: Vec<Message>) -> Vec<Message> {
        let mut passed = Vec::with_capacity(messages.len());
        for message in messages {
            match &message {
                Message::Editor(editor)
                    if !matches!(editor, EditorMessage::Close { .. })
                        && !self.entries.contains_key(&editor.instance()) =>
                {
                    let instance = editor.instance();
                    if !self.gone.contains(&instance) {
                        self.waiting.entry(instance).or_default().push(message);
                    }
                }
                _ => passed.push(message),
            }
        }
        passed
    }

    pub(super) fn take_reported(&mut self) -> Vec<Message> {
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

    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn is_opened(&self, instance: EditorInstanceId) -> bool {
        self.entries
            .get(&instance)
            .is_some_and(|entry| entry.opened)
    }

    pub(super) fn remove(&mut self, instance: EditorInstanceId) -> bool {
        self.waiting.remove(&instance);
        self.gone.insert(instance);
        let Some(entry) = self.entries.remove(&instance) else {
            return false;
        };
        crate::be::want(crate::be::Owner::Editor(instance.0), Vec::new());
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
            crate::be::revisions::release(*preview);
        }
        entry.opened
    }

    fn editable(&self, block: Uuid) -> bool {
        crate::be::access(block).can_edit()
    }

    fn replica_frame(&mut self, instance: EditorInstanceId, block: Uuid, frame: Vec<u8>) -> bool {
        let editable = self.editable(block);
        let viewable = crate::be::may_load(block);
        let Some(entry) = self.entries.get_mut(&instance) else {
            return false;
        };
        let message = crate::be::unframe(&frame).and_then(|bytes| FromEditor::decode(&bytes));
        if let Some(FromEditor::ShowPresence { kind, value }) = &message {
            match value.is_some() {
                true => entry.shown.insert((block, *kind)),
                false => entry.shown.remove(&(block, *kind)),
            };
        }
        if entry.link_mut(block).is_none() {
            if entry.early_frames.len() < EARLY_FRAMES {
                entry.early_frames.push((block, frame));
            }
            return false;
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
        if let Some(source) = crate::be::revisions::source(block) {
            crate::be::revisions::load(block, source, link.content_type);
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
            crate::be::attach(block, link.content_type, instance.0, editable);
        }
        if resend && crate::be::snapshot_frame(block).is_some() {
            link.awaiting_preview = true;
        }
        if !(resend && attaching) {
            crate::be::from_editor(block, instance.0, &frame);
        }
        true
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
                crate::be::provided(*content_type) && crate::be::may_load(*block)
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
                _ => dropped.push((block, link)),
            }
        }
        for (block, content_type) in wanted {
            entry
                .watched
                .entry(block)
                .or_insert_with(|| ContentLink::new(content_type));
        }
        entry.stale = true;
        for (block, mut link) in dropped {
            let shown = entry.shown_on(block);
            link.detach(instance, block, &shown);
            entry.shown.retain(|(on, _)| *on != block);
        }
        let early = std::mem::take(&mut entry.early_frames);
        for (block, frame) in early {
            self.replica_frame(instance, block, frame);
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
                    metrics: viewport_metrics(size, visible, scale_factor, Vec::new()),
                    frame,
                },
                last_seen: pass,
                mounted: 0,
                presented: None,
                holding: false,
                used: None,
                report: None,
                accessibility: Embedded::new(beui::accessibility::next_document_id()),
                focus_requested: false,
                dragging: false,
                file_dropping: false,
                window: WindowRequests::default(),
                stopped: None,
                children: ChildTable::default(),
                reported_statuses: HashMap::new(),
                revoked: HashSet::new(),
                frame_revoked: HashSet::new(),
                scales: Sent::default(),
                serial: 0,
            }
        });
        let monitors = std::mem::take(&mut screen.request.metrics.monitors);
        screen.request.metrics = viewport_metrics(size, visible, scale_factor, monitors);
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
                        && crate::be::may_load(block.id)
                }
                _ => false,
            }
        })
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
        monitors: Vec<Monitor>,
    ) {
        let Some(screen) = self.screen_mut(instance, region) else {
            return;
        };
        let metrics = viewport_metrics(size, visible, scale_factor, monitors);
        if screen.request.metrics != metrics || screen.request.frame != frame {
            screen.request.metrics = metrics;
            screen.request.frame = frame;
        }
    }

    pub(super) fn stopped(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<&Stopped> {
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
            every_sample: false,
            events: vec![block_plugin_api::Input::Back(gesture).into()],
        })]
    }

    pub(super) fn intercepted(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        press: Option<beui::KeyPress>,
        modifiers: beui::Modifiers,
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
        let events = screen.input.intercepted(press, modifiers);
        if events.is_empty() {
            return Vec::new();
        }
        vec![Message::Input(block_plugin_api::InputBatch {
            screen: screen.request.screen,
            every_sample: false,
            events,
        })]
    }

    pub(super) fn intercepted_tap(
        &mut self,
        instance: EditorInstanceId,
        region: EditorRegion,
        key: beui::Key,
    ) -> Vec<Message> {
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
        vec![Message::Input(block_plugin_api::InputBatch {
            screen: screen.request.screen,
            every_sample: false,
            events: vec![block_plugin_api::Input::InterceptedTap(key).into()],
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
        let every_sample = screen
            .report
            .as_ref()
            .is_some_and(|report| report.every_sample);
        let mut messages = screen.input.forward(input, id, zoom, every_sample);
        let files = super::input::file_drop(input, screen.file_dropping);
        messages.extend(self.file_drop(instance, region, files));
        let revoked =
            (escaped || (pressed && input.hovered)) && self.revoke_active(instance, region);
        (messages, revoked)
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
            size: Size {
                width: size.x,
                height: size.y,
            },
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
        editor: Uuid,
        block: EditorBlock,
        source: Uuid,
        data: &[u8],
        resync: bool,
    ) -> Vec<Message> {
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
        self.sent_block_types = None;
        self.announced.clear();
        for entry in self.entries.values_mut() {
            entry.opened = false;
            entry.deferred.clear();
            entry.reported_focus = None;
            entry.reported_editable = None;
            entry.reported_view = None;
            entry.reported_presenting = false;
            entry.host_values.forget_reported();
            entry.reported_history = None;
            entry.heads.forget_sent();
            entry.reported_artifacts.clear();
            entry.reported_size = None;
            entry.sent_blocks.clear();
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
        let graph = crate::be::graph_revision();
        let graph_moved = self.graph_seen.replace(graph) != Some(graph);
        let mut opened = Vec::new();
        let mut screens = Vec::new();
        let plain_forms = be_types_host::plain_forms_revision();
        if self.sent_block_types != Some(plain_forms)
            && let Some(block_types) = &self.block_types
        {
            self.sent_block_types = Some(plain_forms);
            let mut block_types = block_types.as_ref().clone();
            for descriptor in &mut block_types.types {
                descriptor.plain_file =
                    be_types_host::plain_of(Uuid::from_bytes(descriptor.block_type))
                        == be_types_host::Plain::File;
            }
            opened.push(Message::BlockTypes(block_types));
        }
        for instance in instances {
            let Some(entry) = self.entries.get_mut(&instance) else {
                continue;
            };
            crate::be::want(crate::be::Owner::Editor(instance.0), entry.wants());
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
                opened.push(match entry.role {
                    InstanceRole::Editor(editor, block) => Message::Editor(EditorMessage::Open {
                        instance,
                        editor: editor.into_bytes(),
                        block_id: block.id.into_bytes(),
                        block_type: block.block_type.into_bytes(),
                        view_block: block.view_block.map(Uuid::into_bytes),
                        account_id,
                        organization_id,
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
                            editor: editor.into_bytes(),
                            template: template.to_owned(),
                            account_id,
                            organization_id,
                            client_id,
                        })
                    }
                    InstanceRole::Artifact(editor, block, source) => {
                        Message::Editor(EditorMessage::OpenArtifact {
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
                });
                let mut statuses: Vec<_> = entry
                    .screens
                    .values()
                    .flat_map(|screen| screen.reported_statuses.values().cloned())
                    .collect();
                statuses.sort_by_key(|status| status.child.0);
                if !statuses.is_empty() {
                    opened.push(Message::ChildStatuses(statuses));
                }
                let visible = presence_wanted(instance);
                if visible && entry.presence_visible != Some(true) {
                    entry.presence_visible = Some(true);
                    opened.push(Message::Editor(EditorMessage::Presence {
                        instance,
                        visible,
                    }));
                }
            }
            opened.append(&mut entry.deferred);
            entry.relay_frames(instance, &mut opened);
            let stale = std::mem::take(&mut entry.stale);
            if stale || graph_moved {
                entry.reattach(instance);
            }
            if let InstanceRole::Editor(_, block) = entry.role
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
            }
            if (stale || graph_moved)
                && let Some(message) = entry.history_message(instance)
            {
                opened.push(message);
            }
            if stale || graph_moved {
                entry
                    .heads
                    .request(instance, self.epoch, &self.head_reads, &mut opened);
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
            let presenting = entry.presenting || entry.presented;
            if presenting != entry.reported_presenting {
                entry.reported_presenting = presenting;
                opened.push(Message::Editor(EditorMessage::PresentingChanged {
                    instance,
                    presenting,
                }));
            }
            entry.host_values.report(
                instance,
                self.shell == Some(instance),
                &self.published,
                &mut opened,
            );
            if entry.view != entry.reported_view {
                entry.reported_view = entry.view;
                if let Some(view) = entry.view {
                    opened.push(Message::Editor(EditorMessage::ViewChanged {
                        instance,
                        view: view.rect,
                        scale: view.scale,
                    }));
                }
            }
            screens.extend(regions);
        }
        NextScreens { opened, screens }
    }

    pub(super) fn configure(&mut self, messages: &[Message]) -> Option<Message> {
        let configures = messages.iter().any(|message| {
            matches!(
                message,
                Message::Screens(_) | Message::Editor(EditorMessage::ViewChanged { .. })
            )
        });
        if !configures {
            return None;
        }
        self.serial += 1;
        let serial = self.serial;
        for message in messages {
            match message {
                Message::Screens(set) => {
                    for request in &set.screens {
                        if let Some(screen) = self.screen_by_id(request.screen) {
                            screen.scales.push(serial, request.metrics.scale_factor);
                        }
                    }
                }
                Message::Editor(EditorMessage::ViewChanged {
                    instance,
                    view,
                    scale,
                }) => {
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
        Some(Message::Configure { serial })
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
            }
            if let Some(oldest) = oldest {
                entry.views.confirm(oldest);
            }
        }
    }

    pub(super) fn set_region_sizes(&mut self, sizes: Vec<RegionSize>) -> bool {
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
                    pos: event.position,
                    block_id: event.drag.block_id.into_bytes(),
                    block_type: event.drag.block_type.into_bytes(),
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
                    pos: event.position,
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

    pub(super) fn set_children(&mut self, placements: ChildPlacements) -> bool {
        let ChildPlacements {
            instance,
            region,
            children,
            occluders,
        } = placements;
        let Some(screen) = self
            .entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
        else {
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
        let Some(screen) = self
            .entries
            .get(&placements.instance)
            .and_then(|entry| entry.screens.get(&placements.region))
        else {
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
        region: EditorRegion,
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
        let laid_view = entry
            .views
            .at(screen.serial)
            .copied()
            .filter(|_| region == EditorRegion::Frame);
        let mut children = Vec::new();
        let mut holes = Holes::default();
        let mut live = 0;
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
        for (index, child) in table.children.iter().enumerate() {
            if self
                .entries
                .get(&instance)
                .and_then(|entry| entry.role.block())
                .is_some_and(|block| crate::be::revisions::source(block.id).is_some())
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
                                crate::be::revisions::alias(parent.id, Uuid::from_bytes(*block_id))
                            },
                        ),
                    block_type: Uuid::from_bytes(*block_type),
                    view_block: if self
                        .entries
                        .get(&instance)
                        .and_then(|entry| entry.role.block())
                        .is_some_and(|parent| crate::be::revisions::source(parent.id).is_some())
                    {
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
                ChildContent::WebView(_) => continue,
            };
            if !child.rect.is_positive() {
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
            let child_rect = host_rect(child.rect, origin, scale);
            let (view_rect, child_clip) = match (child.follows_view, laid_view, view) {
                (true, Some(laid_view), Some(view)) => (
                    host_rect(through(child.rect, laid_view, view), origin, scale),
                    host_rect(followed_clip(child, laid_view, view), origin, scale),
                ),
                _ => (child_rect, host_rect(child.clip, origin, scale)),
            };
            let child_clip = child_clip.intersect(clip);
            let occluders: Vec<Rect> = table
                .occluders
                .iter()
                .filter(|occluder| occluder.after as usize > index)
                .map(|occluder| host_rect(occluder.rect, origin, scale))
                .collect();
            if matches!(
                mode,
                ChildMode::Active | ChildMode::Live | ChildMode::InPlace
            ) {
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
            });
        }
        (children, holes)
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
                    && child.rect.is_positive()
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
                active: status.active,
                interaction: status.interaction,
                capabilities: status.capabilities,
                resize: status.resize,
                error: status.error,
                menu: status.menu,
                creation: status.creation,
                settings: status.settings,
                preview: status.preview,
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
        pick: super::runtime::Pick,
    ) -> Vec<Message> {
        if !self.entries.contains_key(&instance) {
            return Vec::new();
        }
        vec![Message::Editor(EditorMessage::PickRequested {
            instance,
            pick: pick.pick,
            filter: pick.filter,
            parent: pick.parent,
            within: pick.within.map(Uuid::into_bytes),
            touch: pick.touch,
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
                            logical: vec2(metrics.logical.width, metrics.logical.height),
                            pixels: [metrics.pixel_width, metrics.pixel_height],
                            scale_factor: metrics.scale_factor,
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
        for mut report in reports {
            let mut tree = report
                .description
                .as_mut()
                .and_then(|description| description.tree.take());
            for entry in self.entries.values_mut() {
                if let Some(screen) = entry
                    .screens
                    .values_mut()
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
        region: EditorRegion,
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
        region: EditorRegion,
    ) -> Option<Described<'_>> {
        let screen = self.entries.get_mut(&instance)?.screens.get_mut(&region)?;
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
        region: EditorRegion,
    ) -> bool {
        self.entries
            .get_mut(&instance)
            .and_then(|entry| entry.screens.get_mut(&region))
            .is_some_and(|screen| std::mem::take(&mut screen.focus_requested))
    }

    pub(super) fn drive_web_views(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        for (instance, entry) in &mut self.entries {
            for web_view in entry.unshown_web_views.drain(..) {
                messages.push(Message::Editor(EditorMessage::WebViewEvent {
                    instance: *instance,
                    web_view,
                    event: WebViewEvent::Failed("This app can't show web pages yet.".to_owned()),
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
        self.entries
            .values()
            .flat_map(|entry| entry.screens.values())
            .any(|screen| screen.window.pointer_locked)
    }

    pub(super) fn ime(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
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
        region: EditorRegion,
    ) -> Option<&WindowRequests> {
        let screen = self.entries.get(&instance)?.screens.get(&region)?;
        Some(&screen.window)
    }

    pub(super) fn cursor(
        &self,
        instance: EditorInstanceId,
        region: EditorRegion,
    ) -> Option<beui::CursorIcon> {
        let screen = self.entries.get(&instance)?.screens.get(&region)?;
        Some(screen.window.cursor)
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
                if let HostReply::RevisionPreviewed(Ok(preview)) = &reply.reply {
                    self.entries
                        .get_mut(&reply.instance)
                        .unwrap()
                        .revision_previews
                        .insert(Uuid::from_bytes(*preview));
                }
                messages.push(Message::Editor(EditorMessage::Replied {
                    instance: reply.instance,
                    request_id: reply.request_id,
                    reply: reply.reply,
                }));
            } else if let HostReply::RevisionPreviewed(Ok(preview)) = reply.reply {
                crate::be::revisions::release(Uuid::from_bytes(preview));
            }
        }
        let entries = &self.entries;
        self.head_reads.arrived(
            self.epoch,
            |instance, block, commit| {
                entries
                    .get(&instance)
                    .is_some_and(|entry| entry.heads.wants(block, commit))
            },
            &mut messages,
        );
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
            request @ (HostRequest::ListRevisions(_)
            | HostRequest::PreviewRevision { .. }
            | HostRequest::RestoreRevision { .. }) => {
                let block = match &request {
                    HostRequest::ListRevisions(block) => *block,
                    HostRequest::PreviewRevision { block_id, .. }
                    | HostRequest::RestoreRevision { block_id, .. } => *block_id,
                    _ => unreachable!(),
                };
                crate::be::revisions::request(Uuid::from_bytes(block), request, reply);
            }
            HostRequest::ReleaseRevision(preview) => {
                let preview = Uuid::from_bytes(preview);
                if entry.revision_previews.remove(&preview) {
                    crate::be::revisions::release(preview);
                }
                reply(HostReply::RevisionReleased);
            }
            HostRequest::PickFile(filter) => {
                host::pick_file(filter, move |picked| reply(HostReply::FilePicked(picked)));
            }
            HostRequest::SaveFile(file) => {
                host::save_file(file, move |saved| reply(HostReply::FileSaved(saved)));
            }
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
            HostRequest::ContactLink { block_id, access } => {
                let offer = (Uuid::from_bytes(block_id), super::graph::access_of(access));
                crate::be::make_contact_link(Some(offer), move |made| {
                    reply(HostReply::ContactLinkMade(match made {
                        Ok(link) => block_plugin_api::ContactLinkResult::Made(link),
                        Err(error) => block_plugin_api::ContactLinkResult::Failed(error),
                    }));
                });
            }
            HostRequest::ExportBlock(block) => {
                let block = Uuid::from_bytes(block);
                if !self.can_view(block) {
                    reply(HostReply::FileSaved(FileSave::Failed(
                        "You can't view this block.".to_owned(),
                    )));
                    return true;
                }
                crate::be::export(block, move |exported| match exported {
                    Ok((name, data)) => {
                        let file = block_plugin_api::SavedFile {
                            name,
                            mime_type: be_client::transfer::MIME_TYPE.to_owned(),
                            data,
                        };
                        host::save_file(file, move |saved| reply(HostReply::FileSaved(saved)));
                    }
                    Err(error) => reply(HostReply::FileSaved(FileSave::Failed(error))),
                });
            }
            HostRequest::ReadPlainFile(block) => {
                let block = Uuid::from_bytes(block);
                if !self.can_view(block) {
                    reply(HostReply::PlainFileRead(Err(
                        "You can't view this block.".to_owned()
                    )));
                    return true;
                }
                crate::be::plain_file(block, move |read| {
                    reply(HostReply::PlainFileRead(read.map(|file| {
                        block_plugin_api::SavedFile {
                            name: file.name.unwrap_or_default(),
                            mime_type: file.media_type,
                            data: file.bytes,
                        }
                    })));
                });
            }
            HostRequest::ImportBlocks(parent) => {
                let parent = super::graph::parent_of(parent);
                if parent.block().is_some_and(|parent| !self.editable(parent)) {
                    reply(HostReply::BlocksImported(BlockImport::Failed(
                        "You can't add blocks here.".to_owned(),
                    )));
                    return true;
                }
                let filter = beui::FileFilter {
                    name: "Block file".to_owned(),
                    extensions: vec![be_client::transfer::EXTENSION.to_owned()],
                    mime_types: vec![be_client::transfer::MIME_TYPE.to_owned()],
                };
                host::pick_file(filter, move |picked| {
                    reply(HostReply::BlocksImported(match picked {
                        Ok(Some(file)) => match crate::be::import(&file.data, parent) {
                            Ok((block, block_type)) => BlockImport::Imported {
                                block_id: block.into_bytes(),
                                block_type: block_type.into_bytes(),
                            },
                            Err(error) => BlockImport::Failed(error),
                        },
                        Ok(None) => BlockImport::Cancelled,
                        Err(error) => BlockImport::Failed(error),
                    }));
                });
            }
            HostRequest::CopyBlocks { source, parent } => {
                let source = Uuid::from_bytes(source);
                let parent = super::graph::parent_of(parent);
                if !self.can_view(source)
                    || parent.block().is_some_and(|parent| !self.editable(parent))
                {
                    reply(HostReply::BlocksImported(BlockImport::Failed(
                        "You can't copy that block here.".to_owned(),
                    )));
                    return true;
                }
                crate::be::export(source, move |exported| {
                    reply(HostReply::BlocksImported(match exported {
                        Ok((_, bytes)) => match crate::be::import(&bytes, parent) {
                            Ok((block, block_type)) => BlockImport::Imported {
                                block_id: block.into_bytes(),
                                block_type: block_type.into_bytes(),
                            },
                            Err(error) => BlockImport::Failed(error),
                        },
                        Err(error) => BlockImport::Failed(error),
                    }));
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
        let preview = self
            .entries
            .get(&message.instance())
            .and_then(|entry| entry.role.block())
            .is_some_and(|block| crate::be::revisions::source(block.id).is_some());
        if preview
            && !matches!(
                &message,
                EditorMessage::WatchContent { .. }
                    | EditorMessage::WatchBlocks { .. }
                    | EditorMessage::Replica { .. }
                    | EditorMessage::WatchHistory { .. }
                    | EditorMessage::WatchHeads { .. }
                    | EditorMessage::WatchArtifacts { .. }
                    | EditorMessage::WatchHostValue { .. }
                    | EditorMessage::Focused { .. }
                    | EditorMessage::Menu { .. }
                    | EditorMessage::ChangeView { .. }
                    | EditorMessage::Stopped { .. }
                    | EditorMessage::CopyText { .. }
                    | EditorMessage::CopyImage { .. }
                    | EditorMessage::WebViewCommand { .. }
                    | EditorMessage::Request {
                        request: HostRequest::Fetch(_)
                            | HostRequest::ReadData(_)
                            | HostRequest::ListData,
                        ..
                    }
            )
        {
            return false;
        }
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
            EditorMessage::WatchHostValue { instance, key } => self
                .entries
                .get_mut(&instance)
                .is_some_and(|entry| entry.host_values.watch(key)),
            EditorMessage::HostAction {
                instance,
                key,
                action,
            } => {
                let shell = self.shell == Some(instance);
                self.entries
                    .get_mut(&instance)
                    .is_some_and(|entry| entry.host_values.act(key, action, shell))
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
                editor,
            } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry.opens.push(OpenRequest {
                    id: Uuid::from_bytes(block_id),
                    block_type: Uuid::from_bytes(block_type),
                    via: via.map(Uuid::from_bytes),
                    editor: editor.map(Uuid::from_bytes),
                });
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
            EditorMessage::WatchHeads { instance, blocks } => {
                let Some(entry) = self.entries.get_mut(&instance) else {
                    return false;
                };
                entry
                    .heads
                    .watch(blocks.into_iter().map(Uuid::from_bytes).collect());
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
                entry.block_drags.push(BlockDrag {
                    block_id: Uuid::from_bytes(block_id),
                    block_type: Uuid::from_bytes(block_type),
                });
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
            EditorMessage::WatchContent { instance, blocks } => {
                self.watch_content(instance, blocks)
            }
            EditorMessage::Replica {
                instance,
                block_id,
                frame,
            } => self.replica_frame(instance, Uuid::from_bytes(block_id), frame),
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
                defined,
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
                let mut defined = super::graph::entries_of(defined);
                defined.remove(&be_block::LOCAL_ID.id);
                let metadata = be_block::BlockMetadata {
                    defined,
                    ..be_block::BlockMetadata::default()
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
            EditorMessage::Define {
                block_id,
                key,
                value,
                ..
            } => {
                let block = Uuid::from_bytes(block_id);
                let key = Uuid::from_bytes(key);
                if !self.editable(block) || key == be_block::LOCAL_ID.id {
                    return false;
                }
                crate::be::define(block, vec![(key, value.map(|value| value.into_vec()))]);
                true
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
            EditorMessage::PlayAudio { instance, command } => {
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
                    AudioCommand::Load(source) => player.load(source),
                    AudioCommand::Toggle => player.toggle(),
                    AudioCommand::Reset => player.reset(),
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
                if let WebViewCommand::Open(_) = command {
                    entry.unshown_web_views.push(web_view);
                }
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
            EditorMessage::CopyText { instance, text } => {
                if !self.entries.contains_key(&instance) {
                    return false;
                }
                host::copy_text(text);
                false
            }
            EditorMessage::CopyImage { instance, png } => {
                if !self.entries.contains_key(&instance) {
                    return false;
                }
                host::copy_image(png);
                false
            }
            EditorMessage::Paste { instance } => {
                if !self.entries.contains_key(&instance) {
                    return false;
                }
                host::request_paste();
                false
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
            EditorMessage::Stopped {
                instance,
                region,
                stopped,
            } => {
                let Some(screen) = self
                    .entries
                    .get_mut(&instance)
                    .and_then(|entry| entry.screens.get_mut(&region))
                else {
                    return false;
                };
                screen.stopped = Some(stopped);
                true
            }
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

    pub(super) fn take_block_drags(&mut self, instance: EditorInstanceId) -> Vec<BlockDrag> {
        self.entries
            .get_mut(&instance)
            .map(|entry| std::mem::take(&mut entry.block_drags))
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
        open: OpenRequest,
    ) -> Vec<Message> {
        vec![Message::Editor(EditorMessage::ShowBlock {
            instance,
            block_id: open.id.into_bytes(),
            block_type: open.block_type.into_bytes(),
            via: open.via.map(Uuid::into_bytes),
            editor: open.editor.map(Uuid::into_bytes),
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

    pub(super) fn set_host_value(&mut self, key: &str, value: Arc<Vec<u8>>) -> bool {
        self.published.insert(key.to_owned(), value);
        self.watches_host_value(key)
    }

    pub(super) fn watches_host_value(&self, key: &str) -> bool {
        self.entries.iter().any(|(instance, entry)| {
            entry
                .host_values
                .watches(key, self.shell == Some(*instance))
        })
    }

    pub(super) fn take_host_actions(&mut self, key: &str) -> Vec<Vec<u8>> {
        let mut instances: Vec<_> = self.entries.keys().copied().collect();
        instances.sort_by_key(|instance| instance.0);
        instances
            .into_iter()
            .flat_map(|instance| {
                self.entries
                    .get_mut(&instance)
                    .map(|entry| entry.host_values.take_actions(key))
                    .unwrap_or_default()
            })
            .collect()
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
