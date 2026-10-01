use beui::{Vec2, vec2};
use block_plugin_api::{
    BlockPick, BlockTypeDescriptor, EditorCapabilities, EditorInstanceId, EditorManifest,
    EditorRegion, FrameSpec, InteractionMode, PaneId, PaneLayout, PaneTree, PluginManifest,
    ResizeMode,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use uuid::Uuid;

pub(crate) mod discovery;

use super::{
    ArtifactSession, ArtifactStatus, BlockTypeEntry, CreationStep, DirectEditorCapabilities,
    DirectEditorInteraction, DirectEditorResize, EditorAccess, EditorAction, EditorRegistry,
    FocusReport, PendingCreation,
};
use crate::{
    block_picker::BlockPicker,
    host::{self, Ui},
    plugin_host::{
        ArtifactSlot, ArtifactState, CreationSlot, CreationState, EditorBlock, InstanceRole,
    },
};

pub(super) fn block_type_descriptors(
    types: impl IntoIterator<Item = (uuid::Uuid, BlockTypeEntry)>,
) -> Vec<BlockTypeDescriptor> {
    types
        .into_iter()
        .map(|(block_type, entry)| BlockTypeDescriptor {
            block_type: block_type.into_bytes(),
            display_name: entry.display_name,
            icon_codepoint: entry.icon.to_owned(),
            children: block_plugin_api::ChildOperations {
                add: entry.add,
                delete: entry.delete,
                replace: entry.replace,
            },
        })
        .collect()
}

static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

fn next_instance() -> EditorInstanceId {
    EditorInstanceId(NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone, Copy)]
pub(crate) struct CreationTarget {
    pub(crate) editor: Uuid,
    pub(crate) template: &'static str,
    pub(crate) block_type: Uuid,
    pub(crate) dialog: bool,
    pub(crate) name: &'static str,
}

pub(super) struct PluginCreation {
    plugin: Arc<PluginManifest>,
    target: CreationTarget,
    instance: EditorInstanceId,
    opened: bool,
    state: CreationState,
    committed: bool,
    block_pick: Option<PendingBlockPick>,
}

impl PluginCreation {
    pub(super) fn new(plugin: Arc<PluginManifest>, target: CreationTarget) -> Self {
        Self {
            plugin,
            target,
            instance: next_instance(),
            opened: false,
            state: CreationState::Starting,
            committed: false,
            block_pick: None,
        }
    }

    fn role(&self) -> InstanceRole {
        InstanceRole::Creation(self.target.editor, self.target.template)
    }

    fn dialog_ui(&mut self, ui: &mut Ui, editors: &mut EditorAccess<'_>) {
        let height = self.height().unwrap_or(CREATION_DIALOG_HEIGHT);
        crate::plugin_host::editor_ui(
            ui,
            crate::plugin_host::EditorSlot {
                plugin: &self.plugin,
                block_types: editors.registry().plugin_block_types(),
                client_id: editors.client_id(),
                role: self.role(),
                instance: self.instance,
                region: EditorRegion::Frame,
                frame: Some(FrameSpec::default()),
                size: vec2(ui.rect().width(), height),
                view: None,
            },
        )
        .present(ui);
    }
}

impl Drop for PluginCreation {
    fn drop(&mut self) {
        if self.opened {
            crate::plugin_host::close(&self.plugin.identity.id, self.instance);
        }
    }
}

impl PendingCreation for PluginCreation {
    fn ui(&mut self, ui: &mut Ui, editors: &mut EditorAccess<'_>) -> CreationStep {
        self.opened = true;
        if self.target.dialog {
            self.dialog_ui(ui, editors);
            serve_block_pick(
                &self.plugin.identity.id,
                self.instance,
                &mut self.block_pick,
                editors,
                Vec::new(),
                be_graph::BlockParent::Root,
            );
            let ready = crate::plugin_host::creation_ready(&self.plugin.identity.id, self.instance);
            if ready {
                self.state = CreationState::Ready;
            }
            return CreationStep::Options(ready);
        }
        self.state = crate::plugin_host::creation(CreationSlot {
            plugin: &self.plugin,
            block_types: editors.registry().plugin_block_types(),
            client_id: editors.client_id(),
            instance: self.instance,
            role: self.role(),
        });
        CreationStep::Working
    }

    fn height(&self) -> Option<f32> {
        self.target.dialog.then(|| {
            crate::plugin_host::region_size(
                &self.plugin.identity.id,
                self.instance,
                EditorRegion::Frame,
            )
            .map_or(CREATION_DIALOG_HEIGHT, |size| size.y.max(1.0))
        })
    }

    fn create(&mut self) -> Result<Option<Uuid>, String> {
        match &self.state {
            CreationState::Starting => return Ok(None),
            CreationState::Failed(error) => {
                return Err(format!(
                    "{} could not be created: {error}",
                    self.target.name
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
            Some(Ok(block_id)) => Ok(Some(block_id)),
            Some(Err(error)) => {
                self.committed = false;
                Err(format!(
                    "{} could not be created: {error}",
                    self.target.name
                ))
            }
        }
    }
}

const CREATION_DIALOG_HEIGHT: f32 = 96.0;
const UNSUPPORTED_EDITOR_SIZE: Vec2 = vec2(400.0, 120.0);

pub(crate) struct PluginEditor {
    plugin: Option<Arc<PluginManifest>>,
    id: Uuid,
    block_type: Uuid,
    view_block: Option<Uuid>,
    instance: EditorInstanceId,
    opened: bool,
    block_pick: Option<PendingBlockPick>,
    fullscreen: bool,
    presence_active: bool,
    shown: u32,
}

struct PendingBlockPick {
    request_id: u64,
    picker: BlockPicker,
}

fn serve_block_pick(
    plugin_id: &str,
    instance: EditorInstanceId,
    pending: &mut Option<PendingBlockPick>,
    editors: &mut EditorAccess<'_>,
    excluded: Vec<Uuid>,
    parent: be_graph::BlockParent,
) -> Option<EditorAction> {
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
        if let Some(place) = request.place {
            picker.place_at(place);
        }
        *pending = Some(PendingBlockPick {
            request_id: request.request_id,
            picker,
        });
    }
    let waiting = pending.as_mut()?;
    let picked = waiting.picker.handle(editors, parent);
    let placing = picked.as_ref().and_then(|result| {
        let container = result.into?;
        Some(EditorAction::Command {
            id: result.id,
            command: block_plugin_api::BlockCommand::Place {
                block_type: result.block_type.into_bytes(),
                parent: container.into_bytes(),
                linked: false,
            },
        })
    });
    let pick = match picked {
        Some(result) => Some(BlockPick::Chosen {
            block_id: result.id.into_bytes(),
            block_type: result.block_type.into_bytes(),
            linked: result.linked,
            placed: result.placed,
        }),
        None if waiting.picker.is_open() => None,
        None => Some(BlockPick::Cancelled),
    };
    let pick = pick?;
    let request_id = waiting.request_id;
    *pending = None;
    crate::plugin_host::block_picked(plugin_id, instance, request_id, pick);
    placing
}

impl PluginEditor {
    pub(super) fn new(plugin: Arc<PluginManifest>, id: Uuid, block_type: Uuid) -> Self {
        Self::open(Some(plugin), id, block_type)
    }

    pub(super) fn unsupported(id: Uuid, block_type: Uuid) -> Self {
        Self::open(None, id, block_type)
    }

    fn open(plugin: Option<Arc<PluginManifest>>, id: Uuid, block_type: Uuid) -> Self {
        Self {
            plugin,
            id,
            block_type,
            view_block: None,
            instance: next_instance(),
            opened: false,
            block_pick: None,
            fullscreen: false,
            presence_active: false,
            shown: 0,
        }
    }

    pub(crate) fn viewed_by(mut self, view_block: Option<Uuid>) -> Self {
        self.view_block = view_block;
        self
    }

    fn manifest(&self) -> Option<&EditorManifest> {
        self.plugin.as_ref()?.editor(self.block_type.into_bytes())
    }

    fn capabilities(&self) -> EditorCapabilities {
        self.manifest()
            .map_or_else(EditorCapabilities::default, |editor| editor.capabilities)
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
        if !self.opened {
            return;
        }
        if fullscreen {
            host::set_fullscreen(false);
        }
        if self.presenting() {
            crate::plugin_host::present(&plugin.identity.id, self.instance, false);
            host::request_repaint();
        }
    }

    fn block_pick_ui(&mut self, editors: &mut EditorAccess<'_>) -> Option<EditorAction> {
        let plugin = self.plugin.as_ref()?;
        serve_block_pick(
            &plugin.identity.id,
            self.instance,
            &mut self.block_pick,
            editors,
            vec![self.id],
            be_graph::BlockParent::Block(self.id),
        )
    }

    fn close(&mut self) {
        let Some(plugin) = &self.plugin else {
            return;
        };
        if std::mem::take(&mut self.opened) {
            crate::plugin_host::close(&plugin.identity.id, self.instance);
        }
    }

    pub(crate) fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn plugin(&self) -> Option<&Arc<PluginManifest>> {
        self.plugin.as_ref()
    }

    pub(crate) fn instance(&self) -> EditorInstanceId {
        self.instance
    }

    pub(crate) fn role(&self) -> InstanceRole {
        InstanceRole::Editor(EditorBlock {
            id: self.id,
            block_type: self.block_type,
            view_block: self.view_block,
        })
    }

    pub(crate) fn has_region(&self, region: EditorRegion) -> bool {
        matches!(region, EditorRegion::Pane(_))
            || self
                .manifest()
                .is_some_and(|editor| editor.regions.contains(&region))
    }

    pub(crate) fn serve_block_pick(&mut self, editors: &mut EditorAccess<'_>) -> Option<EditorAction> {
        self.block_pick_ui(editors)
    }

    pub(crate) fn stop_presenting_now(&mut self) {
        self.stop_presenting();
    }

    pub(crate) fn presenting_now(&self) -> bool {
        self.presenting()
    }

    pub(crate) fn shown(&mut self, shown: bool) {
        match shown {
            true => {
                self.opened = true;
                self.shown += 1;
            }
            false => self.shown = self.shown.saturating_sub(1),
        }
        self.sync_active_presence(self.shown > 0);
        if self.shown == 0 {
            self.stop_presenting();
        }
    }

    pub(crate) fn block_type(&self) -> Uuid {
        self.block_type
    }

    pub(crate) fn add_child(&self, child: Uuid) -> Option<bool> {
        self.change_child(be_block::ChildChange::Add(child))
    }

    pub(crate) fn delete_child(&self, child: Uuid) -> Option<bool> {
        self.change_child(be_block::ChildChange::Delete(child))
    }

    pub(crate) fn replace_child(&self, old: Uuid, new: Uuid) -> Option<bool> {
        let replace = be_block::ChildChange::Replace { old, new };
        let Some(plugin) = &self.plugin else {
            return self.change_child(replace);
        };
        if !self
            .manifest()
            .is_some_and(|editor| editor.children.replace)
        {
            return None;
        }
        match crate::plugin_host::replace_child(&plugin.identity.id, self.instance, old, new) {
            Some(true) => Some(true),
            Some(false) => self.change_child(replace),
            None => None,
        }
    }

    fn change_child(&self, change: be_block::ChildChange) -> Option<bool> {
        crate::be::change_child(self.id, self.block_type, change)
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
            max_zoom: capabilities.max_zoom,
        }
    }

    pub(crate) fn direct_editor_interaction(&self) -> DirectEditorInteraction {
        let Some(editor) = self.manifest() else {
            return DirectEditorInteraction::Preview;
        };
        match editor.interaction {
            InteractionMode::Preview => DirectEditorInteraction::Preview,
            InteractionMode::Live => DirectEditorInteraction::Live,
            InteractionMode::Playback => DirectEditorInteraction::Playback,
        }
    }

    pub(crate) fn direct_editor_resize(&self) -> DirectEditorResize {
        let Some(editor) = self.manifest() else {
            return DirectEditorResize::None;
        };
        match editor.resize {
            ResizeMode::None => DirectEditorResize::None,
            ResizeMode::Horizontal => DirectEditorResize::Horizontal,
            ResizeMode::Vertical => DirectEditorResize::Vertical,
            ResizeMode::Both => DirectEditorResize::Both,
        }
    }

    pub(crate) fn direct_editor_intrinsic_size(&mut self) -> Option<Vec2> {
        let Some(plugin) = &self.plugin else {
            return Some(UNSUPPORTED_EDITOR_SIZE);
        };
        Some(
            crate::plugin_host::intrinsic_size(&plugin.identity.id, self.instance)
                .unwrap_or_else(|| vec2(420.0, 240.0)),
        )
    }

    pub(crate) fn show_block(&self, id: Uuid, block_type: Uuid, via: Option<Uuid>) {
        let Some(plugin) = &self.plugin else {
            return;
        };
        crate::plugin_host::show_block(&plugin.identity.id, self.instance, id, block_type, via);
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

    pub(crate) fn panes(&self) -> Option<PaneLayout> {
        let plugin = self.plugin.as_ref()?;
        crate::plugin_host::panes(&plugin.identity.id, self.instance)
    }

    pub(crate) fn take_shown_panes(&self) -> Vec<PaneId> {
        let Some(plugin) = &self.plugin else {
            return Vec::new();
        };
        crate::plugin_host::take_shown_panes(&plugin.identity.id, self.instance)
    }

    pub(crate) fn arrange_panes(
        &self,
        arrangement: u64,
        tree: PaneTree,
        detached: Vec<PaneId>,
        focused: Option<PaneId>,
    ) {
        if let Some(plugin) = &self.plugin {
            crate::plugin_host::arrange_panes(
                &plugin.identity.id,
                self.instance,
                arrangement,
                tree,
                detached,
                focused,
            );
        }
    }

    pub(crate) fn close_pane(&self, pane: PaneId) {
        if let Some(plugin) = &self.plugin {
            crate::plugin_host::close_pane(&plugin.identity.id, self.instance, pane);
        }
    }

    pub(crate) fn pane_more(&self, pane: PaneId) {
        if let Some(plugin) = &self.plugin {
            crate::plugin_host::pane_more(&plugin.identity.id, self.instance, pane);
        }
    }

    pub(crate) fn set_artifact_states(&self, states: Vec<block_plugin_api::ArtifactState>) {
        let Some(plugin) = &self.plugin else {
            return;
        };
        crate::plugin_host::set_artifact_states(&plugin.identity.id, self.instance, states);
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
    source_type: Uuid,
    block: EditorBlock,
    client_id: Uuid,
    instance: EditorInstanceId,
    opened: bool,
    resync: bool,
    outcome: Option<Result<(), String>>,
    regenerating: bool,
}

impl PluginArtifact {
    pub(super) fn new(
        plugin: Arc<PluginManifest>,
        source_type: Uuid,
        target_id: Uuid,
        target_type: Uuid,
        client_id: Uuid,
    ) -> Self {
        Self {
            plugin,
            source_type,
            client_id,
            block: EditorBlock {
                id: target_id,
                block_type: target_type,
                view_block: None,
            },
            instance: next_instance(),
            opened: false,
            resync: false,
            outcome: None,
            regenerating: false,
        }
    }
}

impl Drop for PluginArtifact {
    fn drop(&mut self) {
        if self.opened {
            crate::plugin_host::close(&self.plugin.identity.id, self.instance);
        }
    }
}

impl ArtifactSession for PluginArtifact {
    fn poll(&mut self, registry: &EditorRegistry, data: &[u8]) -> ArtifactStatus {
        self.opened = true;
        let state = crate::plugin_host::artifact(ArtifactSlot {
            plugin: &self.plugin,
            block_types: registry.plugin_block_types(),
            client_id: self.client_id,
            instance: self.instance,
            source_type: self.source_type,
            block: self.block,
            data,
            resync: std::mem::take(&mut self.resync),
        });
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

    fn settings_ui(&mut self, ui: &mut Ui, registry: &EditorRegistry, draft: &mut Vec<u8>) {
        self.opened = true;
        let height = self.settings_height();
        crate::plugin_host::editor_ui(
            ui,
            crate::plugin_host::EditorSlot {
                plugin: &self.plugin,
                block_types: registry.plugin_block_types(),
                client_id: self.client_id,
                role: InstanceRole::Artifact(self.source_type, self.block),
                instance: self.instance,
                region: EditorRegion::ArtifactSettings,
                frame: None,
                size: vec2(ui.rect().width(), height),
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

    fn settings_height(&self) -> f32 {
        crate::plugin_host::region_size(
            &self.plugin.identity.id,
            self.instance,
            EditorRegion::ArtifactSettings,
        )
        .map_or(ARTIFACT_SETTINGS_HEIGHT, |size| size.y.max(1.0))
    }

    fn summary(&self, _draft: &[u8]) -> Option<String> {
        None
    }

    fn cancel_settings(&mut self) {
        self.resync = true;
    }

    fn regenerate(&mut self, data: &[u8]) {
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
