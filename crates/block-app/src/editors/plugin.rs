use block_client::{BlockClient, BlockHandleAccess, blocks, blocks::workspace_index::BlockEntry};
use block_plugin_api::{
    BlockPick, BlockTypeDescriptor, ChildRect, CreationMode, EditorCapabilities, EditorInstanceId,
    EditorRegion, FrameChrome, FrameSpec, InteractionMode, PluginManifest, ResizeMode, ViewChange,
};
use eframe::egui;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use uuid::Uuid;

pub(crate) mod discovery;
mod unsupported;

use super::{
    ArtifactSession, ArtifactStatus, BlockRenderContext, CreationStep, DirectEditorCapabilities,
    DirectEditorInteraction, DirectEditorResize, DirectEditorViewport, DirectEditorViewportCommand,
    DirectEditorViewportInput, EditorAccess, EditorAction, EditorRegistry, FocusReport, FrameSlot,
    PendingCreation, embedded_editor_ui, frame_child_ui, own_frame_child_ui, paint_block_fallback,
    rect_corners,
};
use crate::{
    block_picker::BlockPicker,
    plugin_host::{
        ArtifactSlot, ArtifactState, CreationSlot, CreationState, EditorBlock, EditorView,
        HostChild, HostChildStatus, InstanceRole,
    },
};

use self::unsupported::UnsupportedBlock;

fn child_interaction(editors: &EditorAccess<'_>, block_id: Uuid) -> InteractionMode {
    match editors.direct_editor_interaction(block_id) {
        Some(DirectEditorInteraction::Live) => InteractionMode::Live,
        Some(DirectEditorInteraction::Playback) => InteractionMode::Playback,
        Some(DirectEditorInteraction::Preview) | None => InteractionMode::Preview,
    }
}

fn rotated_corners(rect: egui::Rect, rotation: f32) -> [egui::Pos2; 4] {
    let corners = rect_corners(rect);
    if rotation == 0.0 {
        return corners;
    }
    let (sin, cos) = rotation.sin_cos();
    let center = rect.center();
    corners.map(|corner| {
        let offset = corner - center;
        center
            + egui::vec2(
                offset.x * cos - offset.y * sin,
                offset.x * sin + offset.y * cos,
            )
    })
}

fn collect_view_changes(
    child: block_plugin_api::ChildId,
    viewport: &mut DirectEditorViewport,
    views: &mut Vec<(block_plugin_api::ChildId, ViewChange)>,
) {
    for command in viewport.drain() {
        let change = match command {
            DirectEditorViewportCommand::Pan(delta) => ViewChange::Pan {
                x: delta.x,
                y: delta.y,
            },
            DirectEditorViewportCommand::Zoom { factor, anchor } => ViewChange::Zoom {
                factor,
                anchor: anchor.map(|anchor| (anchor.x, anchor.y)),
            },
            DirectEditorViewportCommand::Fit => ViewChange::Fit,
            DirectEditorViewportCommand::ResumeAutoFit => ViewChange::ResumeAutoFit,
            DirectEditorViewportCommand::AutoFit(_) => continue,
        };
        views.push((child, change));
    }
}

fn child_capabilities(editors: &EditorAccess<'_>, block_id: Uuid) -> EditorCapabilities {
    let Some(capabilities) = editors.direct_editor_capabilities(block_id) else {
        return EditorCapabilities::default();
    };
    EditorCapabilities {
        rotation: capabilities.allow_rotation,
        preserve_aspect_ratio: capabilities.preserve_aspect_ratio,
        pan_and_zoom: capabilities.supports_pan_and_zoom,
    }
}

fn child_resize(editors: &EditorAccess<'_>, block_id: Uuid) -> ResizeMode {
    match editors.direct_editor_resize(block_id) {
        Some(DirectEditorResize::Horizontal) => ResizeMode::Horizontal,
        Some(DirectEditorResize::Vertical) => ResizeMode::Vertical,
        Some(DirectEditorResize::Both) => ResizeMode::Both,
        Some(DirectEditorResize::None) | None => ResizeMode::None,
    }
}

pub(super) fn block_type_descriptors(
    types: impl IntoIterator<Item = (uuid::Uuid, block_ui::BlockTypeEntry)>,
) -> Vec<BlockTypeDescriptor> {
    types
        .into_iter()
        .map(|(block_type, entry)| BlockTypeDescriptor {
            block_type: block_type.into_bytes(),
            display_name: entry.display_name,
            icon_codepoint: entry
                .icon
                .map(|icon| icon.codepoint.to_owned())
                .unwrap_or_default(),
            children: block_plugin_api::ChildOperations {
                add: entry.child_edits.add,
                delete: entry.child_edits.delete,
                replace: entry.child_edits.replace,
            },
        })
        .collect()
}

fn keep_arrow_keys(context: &egui::Context, id: egui::Id) {
    context.memory_mut(|memory| {
        memory.set_focus_lock_filter(
            id,
            egui::EventFilter {
                tab: false,
                horizontal_arrows: true,
                vertical_arrows: true,
                escape: false,
            },
        );
    });
}

static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

fn next_instance() -> EditorInstanceId {
    EditorInstanceId(NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed))
}

pub(super) struct PluginCreation {
    plugin: Arc<PluginManifest>,
    instance: EditorInstanceId,
    context: Option<egui::Context>,
    state: CreationState,
    committed: bool,
    block_pick: Option<PendingBlockPick>,
}

impl PluginCreation {
    pub(super) fn new(plugin: Arc<PluginManifest>) -> Self {
        Self {
            plugin,
            instance: next_instance(),
            context: None,
            state: CreationState::Starting,
            committed: false,
            block_pick: None,
        }
    }

    fn dialog_ui(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) {
        let height = crate::plugin_host::region_size(
            &self.plugin.identity.id,
            self.instance,
            EditorRegion::Frame,
        )
        .map_or(CREATION_DIALOG_HEIGHT, |size| size.y.max(1.0));
        crate::plugin_host::editor_ui(
            ui,
            crate::plugin_host::EditorSlot {
                plugin: &self.plugin,
                block_types: editors.registry().plugin_block_types(),
                client: editors.client_handle(),
                client_id: editors.client_id(),
                role: InstanceRole::Creation,
                instance: self.instance,
                region: EditorRegion::Frame,
                frame: Some(FrameSpec::default()),
                size: egui::vec2(ui.available_width(), height),
                view: None,
            },
        )
        .present(ui);
    }
}

impl Drop for PluginCreation {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            crate::plugin_host::close(&context, &self.plugin.identity.id, self.instance);
        }
    }
}

impl PendingCreation for PluginCreation {
    fn ui(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) -> CreationStep {
        self.context = Some(ui.ctx().clone());
        if self.plugin.creation == CreationMode::Dialog {
            self.dialog_ui(ui, editors);
            serve_block_pick(
                &self.plugin.identity.id,
                self.instance,
                &mut self.block_pick,
                ui,
                editors,
                Vec::new(),
                block::BlockParent::Root,
            );
            let ready = crate::plugin_host::creation_ready(&self.plugin.identity.id, self.instance);
            if ready {
                self.state = CreationState::Ready;
            }
            return CreationStep::Options(ready);
        }
        self.state = crate::plugin_host::creation(
            ui.ctx(),
            CreationSlot {
                plugin: &self.plugin,
                block_types: editors.registry().plugin_block_types(),
                client: editors.client_handle(),
                client_id: editors.client_id(),
                instance: self.instance,
            },
        );
        CreationStep::Working
    }

    fn create(&mut self, client: &BlockClient) -> Result<Option<PluginEditor>, String> {
        match &self.state {
            CreationState::Starting => return Ok(None),
            CreationState::Failed(error) => {
                return Err(format!(
                    "{} could not be created: {error}",
                    self.plugin.display_name
                ));
            }
            CreationState::Ready => {}
        }
        if !self.committed {
            self.committed = true;
            crate::plugin_host::commit_creation(&self.plugin.identity.id, self.instance);
        }
        match crate::plugin_host::take_created(&self.plugin.identity.id, self.instance) {
            None => Ok(None),
            Some(Ok(block_id)) => {
                let block_type = Uuid::from_bytes(self.plugin.block_type);
                let block = blocks::open(client, block_id, block_type)
                    .ok_or_else(|| format!("{block_type} is not a block type this app knows"))?;
                Ok(Some(PluginEditor::new(Arc::clone(&self.plugin), block)))
            }
            Some(Err(error)) => {
                self.committed = false;
                Err(format!(
                    "{} could not be created: {error}",
                    self.plugin.display_name
                ))
            }
        }
    }
}

const CHILD_UNAVAILABLE: &str = "the block is already open above this editor";
const CREATION_DIALOG_HEIGHT: f32 = 96.0;
const UNSUPPORTED_EDITOR_SIZE: egui::Vec2 = egui::vec2(400.0, 120.0);

pub(crate) struct PluginEditor {
    plugin: Option<Arc<PluginManifest>>,
    block: Box<dyn BlockHandleAccess>,
    instance: EditorInstanceId,
    context: Option<egui::Context>,
    block_pick: Option<PendingBlockPick>,
    fullscreen: bool,
    active_this_frame: bool,
    presence_active: bool,
    main_region_id: Option<egui::Id>,
    framed: bool,
}

struct PendingBlockPick {
    request_id: u64,
    picker: BlockPicker,
}

fn serve_block_pick(
    plugin_id: &str,
    instance: EditorInstanceId,
    pending: &mut Option<PendingBlockPick>,
    ui: &mut egui::Ui,
    editors: &mut EditorAccess<'_>,
    excluded: Vec<Uuid>,
    parent: block::BlockParent,
) {
    if pending.is_none()
        && let Some(request) = crate::plugin_host::take_block_pick(plugin_id, instance)
    {
        let mut picker = BlockPicker::default();
        let excluded: Vec<_> = excluded.into_iter().chain(request.excluded).collect();
        if request.templates {
            picker.open_templates_for_types(excluded, request.block_types);
        } else {
            picker.open_for_types(excluded, request.block_types);
        }
        *pending = Some(PendingBlockPick {
            request_id: request.request_id,
            picker,
        });
    }
    let Some(waiting) = pending.as_mut() else {
        return;
    };
    let picked = waiting.picker.handle(ui.ctx(), editors, parent);
    let pick = match picked {
        Some(result) => Some(BlockPick::Chosen {
            block_id: result.id.into_bytes(),
            block_type: result.block_type.into_bytes(),
            linked: result.linked,
        }),
        None if waiting.picker.is_open() => None,
        None => Some(BlockPick::Cancelled),
    };
    let Some(pick) = pick else {
        return;
    };
    let request_id = waiting.request_id;
    *pending = None;
    crate::plugin_host::block_picked(plugin_id, instance, request_id, pick);
}

impl PluginEditor {
    pub(super) fn new(plugin: Arc<PluginManifest>, block: Box<dyn BlockHandleAccess>) -> Self {
        Self::open(Some(plugin), block)
    }

    pub(super) fn unsupported(id: Uuid, block_type: Uuid) -> Self {
        Self::open(None, Box::new(UnsupportedBlock::new(id, block_type)))
    }

    fn open(plugin: Option<Arc<PluginManifest>>, block: Box<dyn BlockHandleAccess>) -> Self {
        Self {
            plugin,
            block,
            instance: next_instance(),
            context: None,
            block_pick: None,
            fullscreen: false,
            active_this_frame: false,
            presence_active: false,
            main_region_id: None,
            framed: false,
        }
    }

    fn capabilities(&self) -> EditorCapabilities {
        self.plugin
            .as_ref()
            .map_or_else(EditorCapabilities::default, |plugin| plugin.capabilities)
    }

    fn sync_active_presence(&mut self, active: bool) {
        let Some(plugin) = &self.plugin else {
            return;
        };
        if active == self.presence_active {
            return;
        }
        self.presence_active = active;
        crate::plugin_host::set_presence_visible(&plugin.identity.id, self.instance, active);
    }

    fn presenting(&self) -> bool {
        self.plugin.as_ref().is_some_and(|plugin| {
            crate::plugin_host::presenting(&plugin.identity.id, self.instance)
        })
    }

    fn stop_presenting(&mut self) {
        let Some(plugin) = &self.plugin else {
            return;
        };
        let fullscreen = std::mem::take(&mut self.fullscreen);
        let Some(context) = self.context.clone() else {
            return;
        };
        if fullscreen {
            context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        if self.presenting() {
            crate::plugin_host::present(&context, &plugin.identity.id, self.instance, false);
            context.request_repaint();
        }
    }

    fn presenting_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let screen = ui.ctx().content_rect();
        let entered = !std::mem::replace(&mut self.fullscreen, true);
        if entered {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
        if ui.ctx().input(|input| input.key_pressed(egui::Key::Escape)) {
            self.stop_presenting();
            return None;
        }
        let area = egui::Id::new(("plugin-presenting", self.instance.0));
        let mut action = None;
        egui::Area::new(area)
            .order(egui::Order::Tooltip)
            .fixed_pos(screen.min)
            .show(ui.ctx(), |ui| {
                ui.set_min_size(screen.size());
                ui.painter().rect_filled(screen, 0.0, egui::Color32::BLACK);
                action = self.frame_ui(ui, editors, FrameSpec::default(), screen.size(), None);
                if entered && let Some(id) = self.main_region_id {
                    ui.ctx().memory_mut(|memory| memory.request_focus(id));
                }
            });
        action
    }

    fn unsupported_ui(&self, ui: &mut egui::Ui) -> Option<EditorAction> {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Unsupported block type");
                ui.label(format!("Block: {}", self.block.id()));
                ui.label(format!("Type: {}", self.block.block_type()));
            });
        });
        None
    }

    fn set_framed(&mut self, framed: bool) {
        if self.framed == framed {
            return;
        }
        self.framed = framed;
        let Some(plugin) = &self.plugin else {
            return;
        };
        crate::plugin_host::hold(&plugin.identity.id, self.instance, EditorRegion::Frame);
    }

    fn frame_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
        chrome: bool,
    ) -> Option<EditorAction> {
        if self.plugin.is_none() {
            return self.unsupported_ui(ui);
        }
        self.set_framed(false);
        self.active_this_frame = true;
        self.context = Some(ui.ctx().clone());
        if self.presenting() {
            return self.presenting_ui(ui, editors);
        }
        self.stop_presenting();
        let rect = ui.available_rect_before_wrap();
        if self.capabilities().pan_and_zoom {
            viewport.auto_fit(self.block.id());
        }
        let view = self.capabilities().pan_and_zoom.then(|| EditorView {
            rect: viewport
                .content_rect()
                .unwrap_or(rect)
                .translate(-rect.min.to_vec2()),
            scale: viewport.scale(),
        });
        let frame = FrameSpec {
            chrome: match chrome {
                true => FrameChrome::Drawn,
                false => FrameChrome::None,
            },
            content: None,
            trail: Vec::new(),
        };
        let action = self.frame_ui(ui, editors, frame, rect.size(), view);
        self.take_view_changes(rect, viewport);
        action
    }

    fn frame_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        frame: FrameSpec,
        size: egui::Vec2,
        view: Option<EditorView>,
    ) -> Option<EditorAction> {
        self.region_ui(ui, editors, EditorRegion::Frame, Some(frame), size, view)
    }

    fn region_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        region: EditorRegion,
        frame: Option<FrameSpec>,
        size: egui::Vec2,
        view: Option<EditorView>,
    ) -> Option<EditorAction> {
        let plugin = self.plugin.clone()?;
        if !plugin.regions.contains(&region) {
            return None;
        }
        self.context = Some(ui.ctx().clone());
        let presentation = crate::plugin_host::editor_ui(
            ui,
            crate::plugin_host::EditorSlot {
                plugin: &plugin,
                block_types: editors.registry().plugin_block_types(),
                client: editors.client_handle(),
                client_id: editors.client_id(),
                role: InstanceRole::Editor(EditorBlock {
                    id: self.block.id(),
                    block_type: self.block.block_type(),
                }),
                instance: self.instance,
                region,
                frame,
                size,
                view,
            },
        );
        if region == EditorRegion::Frame {
            self.main_region_id = presentation.id;
        }
        if let Some(id) = presentation.id {
            keep_arrow_keys(ui.ctx(), id);
        }
        let mut action = presentation
            .command
            .map(|(id, command)| EditorAction::Command { id, command })
            .or_else(|| {
                presentation
                    .drag
                    .map(|(id, block_type)| EditorAction::DragBlock { id, block_type })
            })
            .or_else(|| {
                presentation
                    .open
                    .map(|(id, block_type, via)| EditorAction::OpenBlock {
                        id,
                        block_type,
                        via,
                        from: Some(self.block.id()),
                    })
            });
        let mut statuses = Vec::new();
        let mut views = Vec::new();
        let mut child_viewport = DirectEditorViewport::new();
        child_viewport.set_gestures_read(plugin.capabilities.pan_and_zoom);
        for child in presentation
            .children
            .iter()
            .filter(|child| child.is_below() && !child.frame_owner)
        {
            let next = self.child_ui(
                ui,
                editors,
                child,
                &mut child_viewport,
                &mut statuses,
                &mut views,
            );
            action = action.or(next);
        }
        presentation.present(ui);
        if region == EditorRegion::Frame
            && let Some(rect) = presentation.loading_rect
        {
            let rect = rect.intersect(ui.clip_rect());
            let mut loading = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("plugin-loading", self.instance.0))
                    .max_rect(rect),
            );
            loading.set_clip_rect(rect);
            loading.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.spinner();
                    ui.weak("Loading plugin…");
                });
            });
        }
        for child in presentation
            .children
            .iter()
            .filter(|child| !child.is_below() || child.frame_owner)
        {
            let next = self.child_ui(
                ui,
                editors,
                child,
                &mut child_viewport,
                &mut statuses,
                &mut views,
            );
            action = action.or(next);
        }
        presentation.present_floating(ui);
        presentation.report(statuses);
        crate::plugin_host::report_child_views(&plugin.identity.id, self.instance, region, views);
        if region == EditorRegion::Frame {
            self.block_pick_ui(ui, editors);
        }
        action
    }

    fn child_ui(
        &self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        child: &HostChild,
        viewport: &mut DirectEditorViewport,
        statuses: &mut Vec<HostChildStatus>,
        views: &mut Vec<(block_plugin_api::ChildId, ViewChange)>,
    ) -> Option<EditorAction> {
        editors.ensure(child.block_id, child.block_type);
        let available = editors.is_open(child.block_id);
        if let Some(size) = child.intrinsic {
            editors.set_direct_editor_intrinsic_size(child.block_id, size);
        }
        let hovered = ui
            .ctx()
            .pointer_latest_pos()
            .is_some_and(|position| child.rect.contains(position) && child.clip.contains(position));
        let mut action = None;
        let used: Option<egui::Vec2> = None;
        if available && child.is_preview() {
            let painter = ui.painter().with_clip_rect(child.clip);
            let rendered = editors.render(
                child.block_id,
                BlockRenderContext {
                    painter: &painter,
                    corners: rotated_corners(child.rect, child.rotation),
                    opacity: child.opacity,
                },
            );
            if !rendered {
                paint_block_fallback(&painter, child.rect, child.block_id, editors);
            }
        } else if available && child.frame_owner && child.own_frame {
            action = own_frame_child_ui(
                ui,
                editors,
                child.block_id,
                ("plugin-own-frame-child", self.instance.0, child.child.0),
                child.rect,
                child.clip,
                viewport,
            );
            collect_view_changes(child.child, viewport, views);
        } else if available && child.frame_owner && editors.is_frame_child(ui.ctx(), child.block_id)
        {
            action = frame_child_ui(
                ui,
                editors,
                child.block_id,
                ("plugin-frame-child", self.instance.0, child.child.0),
                child.rect,
                child.clip,
                viewport,
            );
            collect_view_changes(child.child, viewport, views);
        } else if available {
            action = embedded_editor_ui(
                ui,
                editors,
                child.block_id,
                ("plugin-child", self.instance.0, child.child.0),
                child.rect,
                child.clip,
                viewport,
            );
            collect_view_changes(child.child, viewport, views);
        }
        statuses.push(HostChildStatus {
            child: child.child,
            available,
            intrinsic: match used {
                Some(size) => Some(size),
                None => available
                    .then(|| editors.direct_editor_intrinsic_size(child.block_id))
                    .flatten(),
            },
            aspect_ratio: editors.preview_aspect_ratio(child.block_id),
            hovered,
            active: available && child.is_active(),
            interaction: child_interaction(editors, child.block_id),
            capabilities: child_capabilities(editors, child.block_id),
            resize: child_resize(editors, child.block_id),
            error: (!available).then(|| CHILD_UNAVAILABLE.to_owned()),
        });
        action
    }

    fn preview_children_ui(
        &self,
        painter: &egui::Painter,
        editors: &mut EditorAccess<'_>,
        presentation: &crate::plugin_host::PreviewPresentation,
        corners: [egui::Pos2; 4],
        opacity: f32,
    ) {
        let Some(plugin) = &self.plugin else {
            return;
        };
        if presentation.children.is_empty() {
            return;
        }
        let mut statuses = Vec::new();
        for child in &presentation.children {
            editors.ensure(child.block_id, child.block_type);
            let available = editors.is_open(child.block_id);
            if available && child.is_preview() && presentation.drawn {
                let corners = mapped_corners(corners, presentation.size, child.rect);
                editors.render(
                    child.block_id,
                    BlockRenderContext {
                        painter,
                        corners,
                        opacity,
                    },
                );
            }
            statuses.push(HostChildStatus {
                child: child.child,
                available,
                intrinsic: available
                    .then(|| editors.direct_editor_intrinsic_size(child.block_id))
                    .flatten(),
                aspect_ratio: editors.preview_aspect_ratio(child.block_id),
                hovered: false,
                active: false,
                interaction: child_interaction(editors, child.block_id),
                capabilities: child_capabilities(editors, child.block_id),
                resize: child_resize(editors, child.block_id),
                error: (!available).then(|| CHILD_UNAVAILABLE.to_owned()),
            });
        }
        crate::plugin_host::report_children(
            &plugin.identity.id,
            self.instance,
            EditorRegion::Preview,
            statuses,
        );
    }

    fn block_pick_ui(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) {
        let Some(plugin) = &self.plugin else {
            return;
        };
        serve_block_pick(
            &plugin.identity.id,
            self.instance,
            &mut self.block_pick,
            ui,
            editors,
            vec![self.block.id()],
            block::BlockParent::Uuid(self.block.id()),
        );
    }

    fn take_view_changes(&mut self, rect: egui::Rect, viewport: &mut DirectEditorViewport) {
        let Some(plugin) = &self.plugin else {
            return;
        };
        if !plugin.capabilities.pan_and_zoom {
            return;
        }
        for change in crate::plugin_host::take_view_changes(&plugin.identity.id, self.instance) {
            match change {
                ViewChange::Pan { x, y } => viewport.pan(egui::vec2(x, y)),
                ViewChange::Zoom { factor, anchor } => {
                    viewport.change_zoom(factor, anchor.map(|(x, y)| rect.min + egui::vec2(x, y)))
                }
                ViewChange::Fit => viewport.fit(),
                ViewChange::ResumeAutoFit => viewport.resume_auto_fit(),
            }
        }
    }

    fn close(&mut self) {
        let Some(plugin) = &self.plugin else {
            return;
        };
        if let Some(context) = self.context.take() {
            crate::plugin_host::close(&context, &plugin.identity.id, self.instance);
        }
    }

    pub(crate) fn block(&self) -> &dyn BlockHandleAccess {
        self.block.as_ref()
    }

    pub(crate) fn id(&self) -> Uuid {
        self.block.id()
    }

    pub(crate) fn block_type(&self) -> Uuid {
        self.block.block_type()
    }

    pub(crate) fn set_parent(&self, parent: block::BlockParent) {
        self.block.set_parent(parent);
    }

    pub(crate) fn add_child(&self, entry: BlockEntry) -> Option<bool> {
        self.block.add_child(entry.id)
    }

    pub(crate) fn delete_child(&self, entry: BlockEntry) -> Option<bool> {
        self.block.delete_child(entry.id)
    }

    pub(crate) fn replace_child(&self, old: Uuid, new: BlockEntry) -> Option<bool> {
        let Some(plugin) = &self.plugin else {
            return self.block.replace_child(old, new.id);
        };
        if !plugin.children.replace {
            return None;
        }
        match crate::plugin_host::replace_child(&plugin.identity.id, self.instance, old, new.id) {
            Some(true) => Some(true),
            Some(false) => self.block.replace_child(old, new.id),
            None => None,
        }
    }

    pub(crate) fn render(
        &mut self,
        context: BlockRenderContext<'_>,
        editors: &mut EditorAccess<'_>,
    ) -> bool {
        let Some(plugin) = self.plugin.clone() else {
            return false;
        };
        if !plugin.regions.contains(&EditorRegion::Preview) {
            return false;
        }
        self.context = Some(context.painter.ctx().clone());
        let presentation = crate::plugin_host::preview(
            context.painter,
            crate::plugin_host::PreviewSlot {
                plugin: &plugin,
                block_types: editors.registry().plugin_block_types(),
                client: editors.client_handle(),
                client_id: editors.client_id(),
                block_id: self.block.id(),
                block_type: self.block.block_type(),
                instance: self.instance,
                corners: context.corners,
                opacity: context.opacity,
            },
        );
        self.preview_children_ui(
            context.painter,
            editors,
            &presentation,
            context.corners,
            context.opacity,
        );
        presentation.drawn
    }

    pub(crate) fn render_aspect_ratio(&self) -> Option<f32> {
        let plugin = self.plugin.as_ref()?;
        crate::plugin_host::aspect_ratio(&plugin.identity.id, self.instance)
    }

    pub(crate) fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        let capabilities = self.capabilities();
        DirectEditorCapabilities {
            allow_rotation: capabilities.rotation,
            preserve_aspect_ratio: capabilities.preserve_aspect_ratio,
            supports_pan_and_zoom: capabilities.pan_and_zoom,
        }
    }

    pub(crate) fn direct_editor_fills_viewport(&self) -> bool {
        self.capabilities().pan_and_zoom
    }

    pub(crate) fn direct_editor_viewport_input(&self) -> DirectEditorViewportInput {
        if self.capabilities().pan_and_zoom {
            DirectEditorViewportInput::Viewport
        } else {
            DirectEditorViewportInput::Background
        }
    }

    pub(crate) fn direct_editor_interaction(&self) -> DirectEditorInteraction {
        let Some(plugin) = &self.plugin else {
            return DirectEditorInteraction::Preview;
        };
        match plugin.interaction {
            InteractionMode::Preview => DirectEditorInteraction::Preview,
            InteractionMode::Live => DirectEditorInteraction::Live,
            InteractionMode::Playback => DirectEditorInteraction::Playback,
        }
    }

    pub(crate) fn direct_editor_resize(&self) -> DirectEditorResize {
        let Some(plugin) = &self.plugin else {
            return DirectEditorResize::None;
        };
        match plugin.resize {
            ResizeMode::None => DirectEditorResize::None,
            ResizeMode::Horizontal => DirectEditorResize::Horizontal,
            ResizeMode::Vertical => DirectEditorResize::Vertical,
            ResizeMode::Both => DirectEditorResize::Both,
        }
    }

    pub(crate) fn direct_editor_intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let Some(plugin) = &self.plugin else {
            return Some(UNSUPPORTED_EDITOR_SIZE);
        };
        Some(
            crate::plugin_host::intrinsic_size(&plugin.identity.id, self.instance)
                .unwrap_or_else(|| egui::vec2(420.0, 240.0)),
        )
    }

    pub(crate) fn set_direct_editor_intrinsic_size(&mut self, size: egui::Vec2) -> bool {
        let Some(plugin) = &self.plugin else {
            return false;
        };
        crate::plugin_host::resized(&plugin.identity.id, self.instance, size);
        false
    }

    pub(crate) fn direct_editor_owns_frame(&self) -> bool {
        self.plugin.is_some()
    }

    pub(crate) fn show_block(
        &self,
        id: Uuid,
        block_type: Uuid,
        via: Option<Uuid>,
        from: Option<Uuid>,
    ) {
        let Some(plugin) = &self.plugin else {
            return;
        };
        crate::plugin_host::show_block(
            &plugin.identity.id,
            self.instance,
            id,
            block_type,
            via,
            from,
        );
    }

    pub(crate) fn take_focus_report(&self) -> Option<FocusReport> {
        let plugin = self.plugin.as_ref()?;
        crate::plugin_host::take_focus_report(&plugin.identity.id, self.instance).map(|focus| {
            FocusReport {
                block: focus.block,
                via: focus.via,
            }
        })
    }

    pub(crate) fn take_artifact_watch(&self) -> Option<Vec<Uuid>> {
        let plugin = self.plugin.as_ref()?;
        crate::plugin_host::take_artifact_watch(&plugin.identity.id, self.instance)
    }

    pub(crate) fn set_artifact_states(&self, states: Vec<block_plugin_api::ArtifactState>) {
        let Some(plugin) = &self.plugin else {
            return;
        };
        crate::plugin_host::set_artifact_states(&plugin.identity.id, self.instance, states);
    }

    pub(crate) fn direct_editor_frame_child(&mut self) -> Option<Uuid> {
        let plugin = self.plugin.as_ref()?;
        crate::plugin_host::frame_child(&plugin.identity.id, self.instance)
    }

    pub(crate) fn clear_direct_editor_frame_child(&mut self) {
        let Some(plugin) = &self.plugin else {
            return;
        };
        crate::plugin_host::revoke_frame_child(&plugin.identity.id, self.instance);
    }

    pub(crate) fn take_direct_editor_frame_exit(&mut self) -> bool {
        let Some(plugin) = &self.plugin else {
            return false;
        };
        crate::plugin_host::take_leaving(&plugin.identity.id, self.instance)
    }

    pub(crate) fn direct_editor_frame_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        slot: &FrameSlot,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.set_framed(slot.content.is_some());
        self.active_this_frame = true;
        self.context = Some(ui.ctx().clone());
        if self.presenting() {
            return self.presenting_ui(ui, editors);
        }
        self.stop_presenting();
        let rect = slot.frame;
        if self.capabilities().pan_and_zoom {
            viewport.auto_fit(self.block.id());
        }
        let view = self.capabilities().pan_and_zoom.then(|| EditorView {
            rect: viewport
                .content_rect()
                .unwrap_or(rect)
                .translate(-rect.min.to_vec2()),
            scale: viewport.scale(),
        });
        let frame = FrameSpec {
            chrome: match slot.chrome {
                block_ui::frame::Chrome::Drawn => FrameChrome::Drawn,
                block_ui::frame::Chrome::None => FrameChrome::None,
            },
            content: slot.content.map(|content| {
                let content = content.translate(-rect.min.to_vec2());
                ChildRect {
                    x: content.min.x,
                    y: content.min.y,
                    width: content.width(),
                    height: content.height(),
                }
            }),
            trail: slot.trail.clone(),
        };
        let action = self.frame_ui(ui, editors, frame, rect.size(), view);
        self.take_view_changes(rect, viewport);
        if slot.content.is_some()
            && slot.chrome == block_ui::frame::Chrome::Drawn
            && let Some(plugin) = &self.plugin
        {
            crate::plugin_host::cover_frame(&plugin.identity.id, self.instance, rect);
        }
        action
    }

    pub(crate) fn direct_editor_viewport_rect(&self, frame: egui::Rect) -> egui::Rect {
        let Some(plugin) = &self.plugin else {
            return frame;
        };
        crate::plugin_host::frame_rects(&plugin.identity.id, self.instance)
            .map(|rects| rects.content.translate(frame.min.to_vec2()))
            .filter(|content| content.is_positive())
            .unwrap_or(frame)
    }

    pub(crate) fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.frame_editor_ui(ui, editors, viewport, false)
    }

    pub(crate) fn set_tab_active(&mut self, active: bool) {
        if active {
            self.active_this_frame = true;
        }
    }

    pub(crate) fn finish_frame(&mut self) {
        let active = std::mem::take(&mut self.active_this_frame);
        self.sync_active_presence(active);
        if !active {
            self.stop_presenting();
        }
    }

    pub(crate) fn tab_closed(&mut self) {
        self.sync_active_presence(false);
        self.stop_presenting();
        self.close();
    }
}

impl Drop for PluginEditor {
    fn drop(&mut self) {
        self.stop_presenting();
        self.close();
    }
}

pub(super) struct PluginArtifact {
    plugin: Arc<PluginManifest>,
    block: EditorBlock,
    client_id: Uuid,
    instance: EditorInstanceId,
    context: Option<egui::Context>,
    resync: bool,
    outcome: Option<Result<(), String>>,
    regenerating: bool,
}

impl PluginArtifact {
    pub(super) fn new(
        plugin: Arc<PluginManifest>,
        target_id: Uuid,
        target_type: Uuid,
        client_id: Uuid,
    ) -> Self {
        Self {
            plugin,
            client_id,
            block: EditorBlock {
                id: target_id,
                block_type: target_type,
            },
            instance: next_instance(),
            context: None,
            resync: false,
            outcome: None,
            regenerating: false,
        }
    }
}

impl Drop for PluginArtifact {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            crate::plugin_host::close(&context, &self.plugin.identity.id, self.instance);
        }
    }
}

impl ArtifactSession for PluginArtifact {
    fn poll(
        &mut self,
        ctx: &egui::Context,
        registry: &EditorRegistry,
        client: &Arc<BlockClient>,
        data: &[u8],
    ) -> ArtifactStatus {
        self.context = Some(ctx.clone());
        let state = crate::plugin_host::artifact(
            ctx,
            ArtifactSlot {
                plugin: &self.plugin,
                block_types: registry.plugin_block_types(),
                client: Arc::clone(client),
                client_id: self.client_id,
                instance: self.instance,
                block: self.block,
                data,
                resync: std::mem::take(&mut self.resync),
            },
        );
        if let Some(outcome) =
            crate::plugin_host::take_artifact_outcome(&self.plugin.identity.id, self.instance)
        {
            self.regenerating = false;
            self.outcome = Some(outcome);
        }
        match state {
            ArtifactState::Starting => ArtifactStatus::Starting,
            ArtifactState::Described { source, summary } => {
                ArtifactStatus::Described { source, summary }
            }
            ArtifactState::Failed(error) => ArtifactStatus::Failed(error),
        }
    }

    fn settings_ui(
        &mut self,
        ui: &mut egui::Ui,
        registry: &EditorRegistry,
        client: &Arc<BlockClient>,
        draft: &mut Vec<u8>,
    ) {
        self.context = Some(ui.ctx().clone());
        let height = crate::plugin_host::region_size(
            &self.plugin.identity.id,
            self.instance,
            EditorRegion::ArtifactSettings,
        )
        .map_or(ARTIFACT_SETTINGS_HEIGHT, |size| size.y.max(1.0));
        crate::plugin_host::editor_ui(
            ui,
            crate::plugin_host::EditorSlot {
                plugin: &self.plugin,
                block_types: registry.plugin_block_types(),
                client: Arc::clone(client),
                client_id: self.client_id,
                role: InstanceRole::Artifact(self.block),
                instance: self.instance,
                region: EditorRegion::ArtifactSettings,
                frame: None,
                size: egui::vec2(ui.available_width(), height),
                view: None,
            },
        )
        .present(ui);
        if let Some(edited) =
            crate::plugin_host::artifact_draft(&self.plugin.identity.id, self.instance)
        {
            *draft = edited;
        }
    }

    fn summary(&self, _draft: &[u8]) -> Option<String> {
        None
    }

    fn cancel_settings(&mut self) {
        self.resync = true;
    }

    fn regenerate(&mut self, _client: &Arc<BlockClient>, data: &[u8]) {
        self.outcome = None;
        self.regenerating = true;
        crate::plugin_host::regenerate_artifact(&self.plugin.identity.id, self.instance, data);
    }

    fn take_outcome(&mut self) -> Option<Result<(), String>> {
        self.outcome.take()
    }

    fn regenerating(&self) -> bool {
        self.regenerating
    }
}

const ARTIFACT_SETTINGS_HEIGHT: f32 = 72.0;

fn mapped_corners(corners: [egui::Pos2; 4], size: egui::Vec2, rect: egui::Rect) -> [egui::Pos2; 4] {
    let horizontal = corners[1] - corners[0];
    let vertical = corners[3] - corners[0];
    let at = |x: f32, y: f32| {
        corners[0]
            + horizontal * (x / size.x.max(f32::EPSILON))
            + vertical * (y / size.y.max(f32::EPSILON))
    };
    [
        at(rect.min.x, rect.min.y),
        at(rect.max.x, rect.min.y),
        at(rect.max.x, rect.max.y),
        at(rect.min.x, rect.max.y),
    ]
}
