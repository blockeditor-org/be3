use be_block::presence::{PresenceKind, UserActive, pick_free_color};
use block_plugin_api::{
    ArtifactDescription, ChildId, ChildPlacement, ChildPlacements, ChildRect, ChildStatus,
    CreationOutcome, CursorIcon, EditorInstanceId, EditorMessage, EditorRegion, FrameChrome,
    FrameReport, HostReply, ImeArea, InputEvent, MAX_CHILDREN, MAX_COLLECTION_ITEMS, Message,
    Occluder, RegionSize, ScreenPlacement, ScreenRequest, Size, ViewChange, ViewportMetrics,
    WebViewEvent,
};
use block_ui::BlockCatalog;
use geometry::{Rect, Vec2, pos2, vec2};
use std::{collections::HashMap, rc::Rc};
use uuid::Uuid;

#[cfg(target_arch = "wasm32")]
use crate::plugin::PaintTarget;
use crate::plugin::{Frame, Instance, Region};
use crate::{EditorHost, Waker, host::BlockDrag};

pub type Open = fn(EditorHost) -> Box<dyn Instance>;

pub struct EditorSession {
    app: Box<dyn Instance>,
    instance: EditorInstanceId,
    regions: HashMap<EditorRegion, RegionState>,
    host: EditorHost,
    own_block: Option<Uuid>,
    drag: Option<(EditorRegion, BlockDrag)>,
    files: Option<(EditorRegion, crate::host::FileDrop)>,
    intrinsic: Option<Vec2>,
    aspect_ratio: Option<f32>,
    created: Option<CreationOutcome>,
    artifact: Option<ArtifactState>,
    replacements: Vec<(u64, bool)>,
    generation: u64,
}

struct ArtifactState {
    data: Vec<u8>,
    draft: Vec<u8>,
    described: Option<Vec<u8>>,
    edited: bool,
    regenerating: bool,
}

impl ArtifactState {
    fn new(data: Vec<u8>) -> Self {
        Self {
            draft: data.clone(),
            data,
            described: None,
            edited: false,
            regenerating: false,
        }
    }

    fn settings(&mut self, data: Vec<u8>) {
        self.draft.clone_from(&data);
        self.data = data;
        self.edited = false;
    }
}

#[derive(Default)]
struct RegionState {
    placement: Option<ScreenPlacement>,
    metrics: Option<ViewportMetrics>,
    frame: Option<block_plugin_api::FrameSpec>,
    used: Option<Vec2>,
    reported: Option<Vec2>,
    report: Option<FrameReport>,
    reported_frame: Option<FrameReport>,
    cursor: CursorIcon,
    reported_cursor: Option<CursorIcon>,
    ime: Option<ImeArea>,
    reported_ime: Option<Option<ImeArea>>,
    children: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
    reported_children: Option<(Vec<ChildPlacement>, Vec<Occluder>)>,
}

impl EditorSession {
    pub fn new(instance: EditorInstanceId, waker: Waker, open: Open) -> Self {
        let host = EditorHost::new(waker);
        Self::adopt(instance, open(host.clone()), host)
    }

    pub fn adopt(instance: EditorInstanceId, app: Box<dyn Instance>, host: EditorHost) -> Self {
        Self {
            app,
            instance,
            regions: HashMap::new(),
            host,
            own_block: None,
            drag: None,
            files: None,
            intrinsic: None,
            aspect_ratio: None,
            created: None,
            artifact: None,
            replacements: Vec::new(),
            generation: 0,
        }
    }

    pub(crate) fn set_block_types(&self, catalog: Rc<BlockCatalog>) {
        self.host.set_block_types(catalog);
    }

    pub fn instance(&self) -> &dyn Instance {
        self.app.as_ref()
    }

    pub fn instance_mut(&mut self) -> &mut dyn Instance {
        self.app.as_mut()
    }

    pub(crate) fn set_audio(&self, status: block_plugin_api::AudioStatus) {
        self.host.set_audio(status);
    }

    pub(crate) fn set_client_id(&self, client_id: Uuid) {
        self.host.set_client_id(client_id);
    }

    pub(crate) fn set_account_id(&self, account: Uuid) {
        self.host.set_account_id(account);
    }

    pub(crate) fn set_workspace_id(&self, workspace: Uuid) {
        self.host.set_workspace_id(workspace);
    }

    pub(crate) fn set_blocks(&self, query: crate::BlockQuery, blocks: Vec<crate::BlockInfo>) {
        self.host.set_blocks(query, blocks);
    }

    pub(crate) fn set_editable(&self, editable: bool) {
        self.host.set_editable(editable);
    }

    pub(crate) fn set_block_content(
        &self,
        block: Uuid,
        content_type: Uuid,
        bytes: Vec<u8>,
        applied: u64,
    ) {
        match self.own_block == Some(block) {
            true => self.host.set_block_content(content_type, bytes, applied),
            false => self
                .host
                .set_content_of(block, content_type, bytes, applied),
        }
    }

    pub(crate) fn set_peers(&self, block: Uuid, peers: Vec<crate::PeerPresence>) {
        let block = (self.own_block != Some(block)).then_some(block);
        self.host.set_peers(block, peers);
    }

    pub(crate) fn push_content_operations(&self, block: Uuid, operations: Vec<(Vec<u8>, bool)>) {
        match self.own_block == Some(block) {
            true => self.host.push_content_operations(operations),
            false => self.host.push_content_operations_of(block, operations),
        }
    }

    pub(crate) fn set_focused_block(&self, focused: crate::host::FocusedBlock) {
        self.host.set_focused_block(focused);
    }

    pub(crate) fn show_block(&self, block_id: Uuid, block_type: Uuid, via: Option<Uuid>) {
        self.host.show_block(block_id, block_type, via);
    }

    pub(crate) fn set_artifacts(&self, states: Vec<crate::host::ArtifactState>) {
        self.host.set_artifacts(states);
    }

    pub(crate) fn set_version_status(&self, status: &block_plugin_api::VersionStatus) {
        self.host.set_version_status(status.clone());
    }

    pub(crate) fn set_histories(&self, states: &[block_plugin_api::HistoryState]) {
        self.host.set_histories(states.iter().map(|state| {
            (
                Uuid::from_bytes(state.block_id),
                crate::host::BlockHistory {
                    can_undo: state.can_undo,
                    can_redo: state.can_redo,
                },
            )
        }));
    }

    pub(crate) fn set_view(&self, view: Rect, scale: f32) {
        self.host.set_view(view, scale);
    }

    pub(crate) fn resized(&mut self, size: Vec2) {
        self.app.resized(size);
    }

    pub fn presence_visible(&mut self, visible: bool) {
        if self.own_block.is_some() {
            let value = visible.then(|| {
                let peers = self
                    .host
                    .peers_since(None, 0)
                    .map(|(_, peers)| peers)
                    .unwrap_or_default();
                let used = peers
                    .iter()
                    .filter(|peer| peer.kind == UserActive::ID)
                    .filter_map(|peer| serde_json::from_slice::<UserActive>(&peer.value).ok())
                    .map(|user| user.color);
                let color = pick_free_color(used);
                serde_json::to_vec(&UserActive { color }).unwrap_or_default()
            });
            self.host.show_presence(None, UserActive::ID, value);
        }
        self.app.presence_visible(visible);
    }

    pub(crate) fn replace_child(&mut self, request_id: u64, old: Uuid, new: Uuid) {
        let replaced = self.app.replace_child(old, new);
        self.replacements.push((request_id, replaced));
    }

    pub(crate) fn set_presenting(&self, presenting: bool) {
        self.host.set_presenting(presenting);
    }

    pub(crate) fn set_drag(&mut self, drag: Option<(EditorRegion, BlockDrag)>) {
        self.drag = drag;
    }

    pub(crate) fn set_files(&mut self, files: Option<(EditorRegion, crate::host::FileDrop)>) {
        self.files = files;
    }

    pub fn connect(&mut self, block_id: Uuid, block_type: Uuid) {
        self.host.set_block_type(block_type);
        self.own_block = Some(block_id);
        self.app.connect(block_id);
    }

    pub(crate) fn connect_creation(&mut self, template: String) {
        self.host.set_editable(true);
        self.app.connect_creation(template);
    }

    pub(crate) fn connect_artifact(&mut self, block_id: Uuid, block_type: Uuid, data: Vec<u8>) {
        self.artifact = Some(ArtifactState::new(data));
        self.host.set_editable(true);
        self.app.connect_artifact(crate::Artifact {
            block_id,
            block_type,
        });
    }

    pub(crate) fn artifact_settings(&mut self, data: Vec<u8>) {
        if let Some(artifact) = &mut self.artifact {
            artifact.settings(data);
        }
    }

    pub(crate) fn regenerate_artifact(&mut self, data: &[u8]) {
        let Some(artifact) = &mut self.artifact else {
            return;
        };
        artifact.regenerating = true;
        self.app.regenerate_artifact(data);
    }

    pub(crate) fn commit_creation(&mut self) {
        self.created = Some(match self.app.create_block() {
            Ok(block_id) => CreationOutcome::Created(block_id.into_bytes()),
            Err(error) => CreationOutcome::Failed(error),
        });
    }

    pub fn place(&mut self, placements: &[ScreenPlacement], requests: &[ScreenRequest]) {
        self.regions
            .retain(|region, _| placements.iter().any(|it| it.region == *region));
        for placement in placements {
            let Some(request) = requests
                .iter()
                .find(|request| request.screen == placement.screen)
            else {
                continue;
            };
            let state = self.regions.entry(placement.region).or_default();
            state.placement = Some(*placement);
            state.metrics = Some(request.metrics.clone());
            state.frame = request.frame.clone();
        }
    }

    pub(crate) fn outbound(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        let sizes = self.region_sizes();
        if !sizes.is_empty() {
            messages.push(Message::RegionSizes(sizes));
        }
        let frames = self.frame_reports();
        if !frames.is_empty() {
            messages.push(Message::Frames(frames));
        }
        let instance = self.instance;
        for (group, measurements) in self.host.take_performance() {
            messages.push(Message::Editor(EditorMessage::Performance {
                instance,
                group,
                measurements,
            }));
        }
        for (region, cursor) in self.cursors() {
            messages.push(Message::Editor(EditorMessage::Cursor {
                instance,
                region,
                cursor,
            }));
        }
        for (region, area) in self.ime_areas() {
            messages.push(Message::Editor(EditorMessage::Ime {
                instance,
                region,
                area,
            }));
        }
        for command in self.host.take_graph_commands() {
            messages.push(Message::Editor(match command {
                crate::GraphCommand::Create {
                    id,
                    block_type,
                    parent,
                    name,
                    artifact,
                    content,
                } => EditorMessage::CreateBlock {
                    instance,
                    block_id: id.into_bytes(),
                    content_type: block_type.into_bytes(),
                    parent: parent.encode(),
                    name,
                    artifact: artifact.map(|artifact| block_plugin_api::ArtifactSource {
                        source_type: artifact.source_type.into_bytes(),
                        data: artifact.data,
                    }),
                    content: content.map(serde_bytes::ByteBuf::from),
                },
                crate::GraphCommand::SetParent { id, parent } => EditorMessage::SetParent {
                    instance,
                    block_id: id.into_bytes(),
                    parent: parent.encode(),
                },
                crate::GraphCommand::SetName { id, name } => EditorMessage::SetName {
                    instance,
                    block_id: id.into_bytes(),
                    name,
                },
            }));
        }
        if let Some(queries) = self.host.take_block_watch() {
            messages.push(Message::Editor(EditorMessage::WatchBlocks {
                instance,
                queries: queries.into_iter().map(crate::BlockQuery::encode).collect(),
            }));
        }
        for (block, operation) in self.host.take_all_content_operations() {
            let Some(block) = block.or(self.own_block) else {
                continue;
            };
            messages.push(Message::Editor(EditorMessage::Operate {
                instance,
                block_id: block.into_bytes(),
                operation,
            }));
        }
        for block in self.host.take_content_resend_requests() {
            let Some(block) = block.or(self.own_block) else {
                continue;
            };
            messages.push(Message::Editor(EditorMessage::ResendContent {
                instance,
                block_id: block.into_bytes(),
            }));
        }
        if let Some(watched) = self.host.take_content_watch() {
            messages.push(Message::Editor(EditorMessage::WatchContent {
                instance,
                blocks: watched
                    .into_iter()
                    .map(|(block, content_type)| block_plugin_api::WatchedContent {
                        block_id: block.into_bytes(),
                        content_type: content_type.into_bytes(),
                    })
                    .collect(),
            }));
        }
        if let Some(accepted) = self.host.take_drag_accepted() {
            messages.push(Message::Editor(EditorMessage::DragAccepted {
                instance,
                accepted,
            }));
        }
        let intrinsic = self.app.intrinsic_size();
        if intrinsic != self.intrinsic {
            self.intrinsic = intrinsic;
            messages.push(Message::Editor(EditorMessage::IntrinsicSize {
                instance,
                size: intrinsic.map(|size| Size {
                    width: size.x,
                    height: size.y,
                }),
            }));
        }
        let aspect_ratio = self.app.aspect_ratio();
        if aspect_ratio != self.aspect_ratio {
            self.aspect_ratio = aspect_ratio;
            messages.push(Message::Editor(EditorMessage::AspectRatio {
                instance,
                ratio: aspect_ratio,
            }));
        }
        if let Some(ready) = self.host.take_creation_ready() {
            messages.push(Message::Editor(EditorMessage::CreationReady {
                instance,
                ready,
            }));
        }
        if let Some(artifact) = &mut self.artifact {
            if artifact.described.as_deref() != Some(artifact.data.as_slice()) {
                artifact.described = Some(artifact.data.clone());
                let description = match self.app.describe_artifact(&artifact.data) {
                    Ok(description) => ArtifactDescription::Described {
                        source: description.source.into_bytes(),
                        summary: description.summary,
                    },
                    Err(error) => ArtifactDescription::Unreadable(error),
                };
                messages.push(Message::Editor(EditorMessage::ArtifactDescribed {
                    instance,
                    description,
                }));
            }
            if artifact.edited {
                artifact.edited = false;
                messages.push(Message::Editor(EditorMessage::ArtifactEdited {
                    instance,
                    data: artifact.draft.clone(),
                }));
            }
            if artifact.regenerating
                && let Some(result) = self.app.poll_artifact()
            {
                artifact.regenerating = false;
                messages.push(Message::Editor(EditorMessage::ArtifactRegenerated {
                    instance,
                    outcome: match result {
                        Ok(()) => block_plugin_api::RegenerationOutcome::Done,
                        Err(error) => block_plugin_api::RegenerationOutcome::Failed(error),
                    },
                }));
            }
        }
        if self.host.take_leave_frame() {
            messages.push(Message::Editor(EditorMessage::LeaveFrame { instance }));
        }
        for shown in self.host.take_shown_presence() {
            let Some(block) = shown.block.or(self.own_block) else {
                continue;
            };
            messages.push(Message::Editor(EditorMessage::ShowPresence {
                instance,
                block_id: block.into_bytes(),
                kind: shown.kind.into_bytes(),
                value: shown.value.map(serde_bytes::ByteBuf::from),
            }));
        }
        for seeded in self.host.take_seeded_content() {
            let (block_id, content_type, bytes) = (
                seeded.block.into_bytes(),
                seeded.content_type.into_bytes(),
                seeded.bytes,
            );
            messages.push(Message::Editor(match seeded.replace {
                true => EditorMessage::ReplaceContent {
                    instance,
                    block_id,
                    content_type,
                    bytes,
                },
                false => EditorMessage::SeedContent {
                    instance,
                    block_id,
                    content_type,
                    bytes,
                },
            }));
        }
        if let Some(outcome) = self.created.take() {
            messages.push(Message::Editor(EditorMessage::CreationBlock {
                instance,
                outcome,
            }));
        }
        for text in self.host.take_copied_text() {
            messages.push(Message::Editor(EditorMessage::CopyText { instance, text }));
        }
        if self.host.take_paste_request() {
            messages.push(Message::Editor(EditorMessage::PasteText { instance }));
        }
        for (request_id, replaced) in std::mem::take(&mut self.replacements) {
            messages.push(Message::Editor(EditorMessage::ChildReplaced {
                instance,
                request_id,
                replaced,
            }));
        }
        for (request_id, request) in self.host.take_requests() {
            messages.push(Message::Editor(EditorMessage::Request {
                instance,
                request_id,
                request,
            }));
        }
        for (block_id, command) in self.host.take_audio_commands() {
            messages.push(Message::Editor(EditorMessage::PlayAudio {
                instance,
                block_id: block_id.into_bytes(),
                command,
            }));
        }
        for (region, rect) in self.host.take_web_view_placements() {
            messages.push(Message::Editor(EditorMessage::WebView {
                instance,
                region,
                rect,
            }));
        }
        for command in self.host.take_web_view_commands() {
            messages.push(Message::Editor(EditorMessage::WebViewCommand {
                instance,
                command,
            }));
        }
        if let Some(grabbed) = self.host.take_cursor_grab() {
            messages.push(Message::Editor(EditorMessage::GrabCursor {
                instance,
                grabbed,
            }));
        }
        for placements in self.children() {
            messages.push(Message::Children(placements));
        }
        self.retain_child_statuses();
        for presenting in self.host.take_present_requests() {
            messages.push(Message::Editor(EditorMessage::Present {
                instance,
                presenting,
            }));
        }
        for change in self.host.take_view_changes() {
            messages.push(Message::Editor(EditorMessage::ChangeView {
                instance,
                change,
            }));
        }
        for (block_id, block_type, via) in self.host.take_opens() {
            messages.push(Message::Editor(EditorMessage::OpenBlock {
                instance,
                block_id: block_id.into_bytes(),
                block_type: block_type.into_bytes(),
                via: via.map(Uuid::into_bytes),
            }));
        }
        for (block_id, block_type) in self.host.take_block_drags() {
            messages.push(Message::Editor(EditorMessage::DragBlock {
                instance,
                block_id: block_id.into_bytes(),
                block_type: block_type.into_bytes(),
            }));
        }
        for (block_id, command) in self.host.take_block_commands() {
            messages.push(Message::Editor(EditorMessage::BlockCommand {
                instance,
                block_id: block_id.into_bytes(),
                command,
            }));
        }
        for (block_id, command) in self.host.take_version_commands() {
            messages.push(Message::Editor(EditorMessage::VersionControl {
                instance,
                block_id: block_id.into_bytes(),
                command,
            }));
        }
        if let Some(focused) = self.host.take_focus_report() {
            messages.push(Message::Editor(EditorMessage::Focused {
                instance,
                block_id: focused.block_id.map(Uuid::into_bytes),
                block_type: focused.block_type.into_bytes(),
                via: focused.via.into_iter().map(Uuid::into_bytes).collect(),
            }));
        }
        if let Some(blocks) = self.host.take_artifact_watch() {
            messages.push(Message::Editor(EditorMessage::WatchArtifacts {
                instance,
                blocks: blocks.into_iter().map(Uuid::into_bytes).collect(),
            }));
        }
        if let Some(blocks) = self.host.take_history_watch() {
            messages.push(Message::Editor(EditorMessage::WatchHistory {
                instance,
                blocks: blocks.into_iter().map(Uuid::into_bytes).collect(),
            }));
        }
        messages
    }

    fn cursors(&mut self) -> Vec<(EditorRegion, CursorIcon)> {
        let mut cursors = Vec::new();
        for (region, state) in &mut self.regions {
            if state.reported_cursor == Some(state.cursor) {
                continue;
            }
            state.reported_cursor = Some(state.cursor);
            cursors.push((*region, state.cursor));
        }
        cursors
    }

    fn ime_areas(&mut self) -> Vec<(EditorRegion, Option<ImeArea>)> {
        let mut areas = Vec::new();
        for (region, state) in &mut self.regions {
            if state.reported_ime == Some(state.ime) {
                continue;
            }
            state.reported_ime = Some(state.ime);
            areas.push((*region, state.ime));
        }
        areas
    }

    fn frame_reports(&mut self) -> Vec<FrameReport> {
        let mut reports = Vec::new();
        for state in self.regions.values_mut() {
            let Some(report) = &state.report else {
                continue;
            };
            if state.reported_frame.as_ref() == Some(report) {
                continue;
            }
            state.reported_frame = Some(report.clone());
            reports.push(report.clone());
        }
        reports
    }

    fn region_sizes(&mut self) -> Vec<RegionSize> {
        let mut sizes = Vec::new();
        for state in self.regions.values_mut() {
            let (Some(placement), Some(used)) = (state.placement, state.used) else {
                continue;
            };
            if state.reported == Some(used) {
                continue;
            }
            state.reported = Some(used);
            sizes.push(RegionSize {
                screen: placement.screen,
                logical_width: used.x,
                logical_height: used.y,
            });
        }
        sizes
    }

    fn used(&mut self, region: EditorRegion, content: Rect) {
        let origin = self.rect(region).min;
        let Some(state) = self.regions.get_mut(&region) else {
            return;
        };
        let used = (content.max - origin).max(Vec2::ZERO);
        state.used = Some(vec2(used.x.round(), used.y.round()));
    }

    pub(crate) fn replied(&self, request_id: u64, reply: HostReply) {
        self.host.set_reply(request_id, reply);
    }

    pub(crate) fn web_view_event(&self, event: WebViewEvent) {
        self.host.push_web_view_event(event);
    }

    pub(crate) fn child_view_change(&self, child: ChildId, change: ViewChange) {
        self.host.push_child_view_change(child, change);
    }

    pub(crate) fn set_child_statuses(&self, statuses: Vec<ChildStatus>) {
        self.host.set_child_statuses(statuses);
    }

    fn children(&mut self) -> Vec<ChildPlacements> {
        let instance = self.instance;
        let generation = self.generation;
        let mut messages = Vec::new();
        for (region, state) in &mut self.regions {
            let current = bounded(&state.children, &state.occluders);
            if state.reported_children.as_ref() == Some(&current) {
                continue;
            }
            state.reported_children = Some(current.clone());
            let (children, occluders) = current;
            messages.push(ChildPlacements {
                instance,
                region: *region,
                generation,
                children,
                occluders,
            });
        }
        messages
    }

    fn retain_child_statuses(&self) {
        let live: Vec<ChildId> = self
            .regions
            .values()
            .flat_map(|state| state.children.iter().map(|child| child.child))
            .collect();
        self.host.retain_child_statuses(&live);
    }

    fn scale_factor(&self, region: EditorRegion) -> f32 {
        self.placement(region)
            .map_or(1.0, |placement| placement.scale_factor())
    }

    fn placement(&self, region: EditorRegion) -> Option<&ScreenPlacement> {
        self.regions
            .get(&region)
            .and_then(|state| state.placement.as_ref())
    }

    fn rect(&self, region: EditorRegion) -> Rect {
        let state = self.regions.get(&region);
        let (Some(placement), Some(metrics)) = (
            state.and_then(|state| state.placement.as_ref()),
            state.and_then(|state| state.metrics.as_ref()),
        ) else {
            return Rect::ZERO;
        };
        let scale = placement.scale_factor();
        Rect::from_min_size(
            pos2(
                placement.x as f32 / scale - metrics.visible_x,
                placement.y as f32 / scale - metrics.visible_y,
            ),
            vec2(metrics.logical_width, metrics.logical_height),
        )
    }

    fn context(&self, region: EditorRegion) -> Region {
        Region {
            region,
            rect: self.rect(region),
            scale_factor: self.scale_factor(region),
            spec: self
                .regions
                .get(&region)
                .and_then(|state| state.frame.clone())
                .unwrap_or_default(),
        }
    }

    pub fn run(&mut self, region: EditorRegion, generation: u64) -> Frame {
        self.generation = generation;
        let context = self.context(region);
        let host = context.rect;
        let origin = host.min.to_vec2();
        let mut draft = self
            .artifact
            .as_ref()
            .filter(|_| region == EditorRegion::ArtifactSettings)
            .map(|artifact| artifact.draft.clone());
        self.host.begin_region(region, origin);
        self.host
            .set_chrome_shown(context.spec.chrome == FrameChrome::Drawn);
        let drag = self.drag.and_then(|(dragged, drag)| {
            (dragged == region).then(|| BlockDrag {
                position: drag.position + origin,
                ..drag
            })
        });
        self.host.set_drag(drag);
        let files = self.files.as_ref().and_then(|(dropped, files)| {
            (*dropped == region).then(|| crate::host::FileDrop {
                position: files.position + origin,
                ..files.clone()
            })
        });
        let delivered_drop = drag.is_some_and(|drag| drag.dropped);
        let delivered_files = files.as_ref().is_some_and(|files| files.dropped);
        self.host.set_files(files);

        let mut frame = self.app.update(&context, draft.as_mut());

        if let Some(delay) = self.host.take_frame_request() {
            frame.repaint_after = Some(frame.repaint_after.map_or(delay, |held| held.min(delay)));
        }
        if let (Some(artifact), Some(draft)) = (self.artifact.as_mut(), draft) {
            artifact.edited |= artifact.draft != draft;
            artifact.draft = draft;
        }
        let (placed, occluders) = self.host.end_region(region);
        self.host.set_drag(None);
        self.host.set_files(None);
        if delivered_drop {
            self.drag = None;
        }
        if delivered_files {
            self.files = None;
        }
        let screen = self.placement(region).map(|placement| placement.screen);
        let content = frame.content.unwrap_or(host);
        self.used(region, host);
        let reported = |rect: Rect| plugin_rect(rect, origin);
        let reported_content = self
            .host
            .take_content()
            .map(|reported| reported.intersect(content))
            .filter(|reported| reported.is_positive())
            .unwrap_or(content);
        if let (Some(state), Some(screen)) = (self.regions.get_mut(&region), screen) {
            state.children = placed;
            state.occluders = occluders;
            state.cursor = frame.cursor;
            state.ime = frame.ime.map(|ime| ImeArea {
                rect: reported(ime.rect),
                cursor: reported(ime.cursor),
            });
            state.report = (region == EditorRegion::Frame).then(|| FrameReport {
                screen,
                content: reported(reported_content),
                painted: frame.painted.iter().map(|rect| reported(*rect)).collect(),
                floating: frame.floating.iter().map(|rect| reported(*rect)).collect(),
            });
        }
        frame
    }

    #[cfg(target_arch = "wasm32")]
    pub fn paint(&mut self, target: &PaintTarget<'_>) {
        self.app.paint(target);
    }

    pub(crate) fn input(&mut self, region: EditorRegion, event: &InputEvent) {
        let context = self.context(region);
        self.app.input(&context, event);
    }

    pub fn report(&self, region: EditorRegion) -> Option<&FrameReport> {
        self.regions
            .get(&region)
            .and_then(|state| state.report.as_ref())
    }

    pub fn placed_children(&self, region: EditorRegion) -> &[ChildPlacement] {
        self.regions
            .get(&region)
            .map_or(&[], |state| state.children.as_slice())
    }

    pub fn occluders(&self, region: EditorRegion) -> &[Occluder] {
        self.regions
            .get(&region)
            .map_or(&[], |state| state.occluders.as_slice())
    }

    pub fn host(&self) -> &EditorHost {
        &self.host
    }
}

fn plugin_rect(rect: Rect, origin: Vec2) -> ChildRect {
    let rect = rect.translate(-origin);
    ChildRect {
        x: rect.min.x,
        y: rect.min.y,
        width: rect.width(),
        height: rect.height(),
    }
}

fn bounded(
    children: &[ChildPlacement],
    occluders: &[Occluder],
) -> (Vec<ChildPlacement>, Vec<Occluder>) {
    let kept = children.len().min(MAX_CHILDREN);
    (
        children[..kept].to_vec(),
        occluders
            .iter()
            .take(MAX_COLLECTION_ITEMS)
            .map(|occluder| Occluder {
                after: occluder.after.min(kept as u32),
                rect: occluder.rect,
            })
            .collect(),
    )
}
