use beui::{Vec2, vec2};
use block_plugin_api::{
    BlockPick, DialogAnswer, DialogStatus, EditorCapabilities, EditorInstanceId, EditorManifest,
    EditorRegion, FrameSpec, HostPanel, InteractionMode, PluginManifest, ResizeMode,
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use uuid::Uuid;

pub(crate) mod discovery;

use super::{
    ArtifactSession, ArtifactStatus, DirectEditorCapabilities, DirectEditorInteraction,
    DirectEditorResize, EditorRegistry, FocusReport, PendingCreation,
};
use crate::{
    compositor::RegionEditor,
    host,
    plugin_host::{
        ArtifactSlot, ArtifactState, CreationSlot, CreationState, EditorBlock, InstanceRole,
    },
    surfaces::HostedRegion,
};

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
    dismissed: bool,
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
            dismissed: false,
        }
    }

    fn role(&self) -> InstanceRole {
        InstanceRole::Creation(self.target.editor, self.target.template)
    }

    fn hosted(&self, registry: &EditorRegistry, client_id: Uuid) -> HostedRegion {
        HostedRegion {
            editor: RegionEditor {
                plugin: Arc::clone(&self.plugin),
                role: self.role(),
                instance: self.instance,
                block_types: Arc::clone(registry.plugin_block_types()),
                client_id,
            },
            region: EditorRegion::Frame,
            frame: Some(FrameSpec::default()),
        }
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
    fn region(&self, registry: &EditorRegistry, client_id: Uuid) -> Option<HostedRegion> {
        self.target.dialog.then(|| self.hosted(registry, client_id))
    }

    fn pick_source(&self) -> PickSource {
        PickSource {
            plugin_id: self.plugin.identity.id.clone(),
            instance: self.instance,
        }
    }

    fn step(&mut self, registry: &EditorRegistry, client_id: Uuid) {
        self.opened = true;
        if self.target.dialog {
            if self.dialog().answer == Some(DialogAnswer::Accepted) {
                self.state = CreationState::Ready;
            }
            return;
        }
        self.state = crate::plugin_host::creation(CreationSlot {
            plugin: &self.plugin,
            block_types: registry.plugin_block_types(),
            client_id,
            instance: self.instance,
            role: self.role(),
        });
    }

    fn dialog(&self) -> DialogStatus {
        let mut status = crate::plugin_host::dialog_status(&self.plugin.identity.id, self.instance)
            .unwrap_or_default();
        if self.dismissed && status.answer.is_none() {
            status.answer = Some(DialogAnswer::Dismissed);
        }
        status
    }

    fn dismiss(&mut self) {
        match crate::plugin_host::dialog_status(&self.plugin.identity.id, self.instance) {
            Some(status) if status.handles_dismiss => {
                crate::plugin_host::dismiss(&self.plugin.identity.id, self.instance);
            }
            _ => self.dismissed = true,
        }
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
    plugin: Option<(Arc<PluginManifest>, Uuid)>,
    id: Uuid,
    block_type: Uuid,
    view_block: Option<Uuid>,
    instance: EditorInstanceId,
    opened: bool,
    presence_active: bool,
    shown: u32,
}

#[derive(Clone)]
pub(crate) struct PickSource {
    pub(crate) plugin_id: String,
    pub(crate) instance: EditorInstanceId,
}

impl PickSource {
    pub(crate) fn take(&self) -> Option<crate::plugin_host::BlockPickRequest> {
        crate::plugin_host::take_block_pick(&self.plugin_id, self.instance)
    }

    pub(crate) fn answer(&self, request_id: u64, pick: BlockPick) {
        crate::plugin_host::block_picked(&self.plugin_id, self.instance, request_id, pick);
    }
}

impl PluginEditor {
    pub(super) fn new(
        plugin: Arc<PluginManifest>,
        editor: Uuid,
        id: Uuid,
        block_type: Uuid,
    ) -> Self {
        Self::open(Some((plugin, editor)), id, block_type)
    }

    pub(super) fn unsupported(id: Uuid, block_type: Uuid) -> Self {
        Self::open(None, id, block_type)
    }

    fn open(plugin: Option<(Arc<PluginManifest>, Uuid)>, id: Uuid, block_type: Uuid) -> Self {
        Self {
            plugin,
            id,
            block_type,
            view_block: None,
            instance: next_instance(),
            opened: false,
            presence_active: false,
            shown: 0,
        }
    }

    pub(crate) fn viewed_by(mut self, view_block: Option<Uuid>) -> Self {
        self.view_block = view_block;
        self
    }

    fn manifest(&self) -> Option<&EditorManifest> {
        let (plugin, editor) = self.plugin.as_ref()?;
        plugin.editor(editor.into_bytes())
    }

    fn capabilities(&self) -> EditorCapabilities {
        self.manifest()
            .map_or_else(EditorCapabilities::default, |editor| editor.capabilities)
    }

    fn sync_active_presence(&mut self, active: bool) {
        if self.plugin.is_none() || active == self.presence_active {
            return;
        }
        self.presence_active = active;
        if let Some(plugin) = self.plugin() {
            crate::plugin_host::set_presence_visible(&plugin.identity.id, self.instance, active);
        }
    }

    fn presenting(&self) -> bool {
        self.plugin().is_some_and(|plugin| {
            crate::plugin_host::presenting(&plugin.identity.id, self.instance)
        })
    }

    fn stop_presenting(&mut self) {
        let Some(plugin) = self.plugin() else {
            return;
        };
        if !self.opened {
            return;
        }
        if self.presenting() {
            crate::plugin_host::present(&plugin.identity.id, self.instance, false);
            host::request_repaint();
        }
    }

    pub(crate) fn pick_source(&self) -> Option<PickSource> {
        let plugin = self.plugin()?;
        Some(PickSource {
            plugin_id: plugin.identity.id.clone(),
            instance: self.instance,
        })
    }

    fn close(&mut self) {
        self.opened = false;
        if let Some(plugin) = self.plugin() {
            crate::plugin_host::close(&plugin.identity.id, self.instance);
        }
    }

    pub(crate) fn id(&self) -> Uuid {
        self.id
    }

    pub(crate) fn plugin(&self) -> Option<&Arc<PluginManifest>> {
        self.plugin.as_ref().map(|(plugin, _)| plugin)
    }

    pub(crate) fn editor_id(&self) -> Option<Uuid> {
        self.plugin.as_ref().map(|(_, editor)| *editor)
    }

    pub(crate) fn instance(&self) -> EditorInstanceId {
        self.instance
    }

    pub(crate) fn role(&self) -> InstanceRole {
        InstanceRole::Editor(
            self.editor_id().unwrap_or_default(),
            EditorBlock {
                id: self.id,
                block_type: self.block_type,
                view_block: self.view_block,
            },
        )
    }

    pub(crate) fn accepts(&self, request: block_plugin_api::ShellRequest) -> bool {
        self.manifest()
            .is_some_and(|editor| editor.accepts(request))
    }

    pub(crate) fn has_region(&self, region: EditorRegion) -> bool {
        self.manifest()
            .is_some_and(|editor| editor.regions.contains(&region))
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
        let Some(plugin) = self.plugin() else {
            return self.change_child(replace);
        };
        if !discovery::block_type(self.block_type)
            .is_some_and(|block_type| block_type.children.replace)
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
        let plugin = self.plugin()?;
        crate::plugin_host::aspect_ratio(&plugin.identity.id, self.instance)
    }

    pub(crate) fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        let capabilities = self.capabilities();
        DirectEditorCapabilities {
            allow_rotation: capabilities.rotation,
            preserve_aspect_ratio: capabilities.preserve_aspect_ratio,
            supports_pan_and_zoom: capabilities.pan_and_zoom,
            max_zoom: capabilities.max_zoom,
            pan: capabilities.pan,
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
        let Some(plugin) = self.plugin() else {
            return Some(UNSUPPORTED_EDITOR_SIZE);
        };
        Some(
            crate::plugin_host::intrinsic_size(&plugin.identity.id, self.instance)
                .unwrap_or_else(|| vec2(420.0, 240.0)),
        )
    }

    pub(crate) fn show_block(&self, open: super::BlockOpen) {
        let Some(plugin) = self.plugin() else {
            return;
        };
        crate::plugin_host::show_block(plugin, self.instance, open);
    }

    pub(crate) fn show_dialog(&self, id: Uuid, dialog: block_plugin_api::ShellDialog) {
        if let Some(plugin) = self.plugin() {
            crate::plugin_host::show_dialog(&plugin.identity.id, self.instance, id, dialog);
        }
    }

    pub(crate) fn show_panel(&self, panel: HostPanel) {
        if let Some(plugin) = self.plugin() {
            crate::plugin_host::show_panel(&plugin.identity.id, self.instance, panel);
        }
    }

    pub(crate) fn claim_shell(&self) {
        crate::plugin_host::set_shell(
            self.plugin()
                .map(|plugin| (plugin.identity.id.as_str(), self.instance)),
        );
    }

    pub(crate) fn take_focus_report(&self) -> Option<FocusReport> {
        let plugin = self.plugin()?;
        crate::plugin_host::take_focus_report(&plugin.identity.id, self.instance).map(|focus| {
            FocusReport {
                block: focus.block,
                via: focus.via,
            }
        })
    }

    pub(crate) fn take_artifact_watch(&self) -> Option<Vec<Uuid>> {
        let plugin = self.plugin()?;
        crate::plugin_host::take_artifact_watch(&plugin.identity.id, self.instance)
    }

    pub(crate) fn menu(&self) -> Vec<block_plugin_api::MenuEntry> {
        match self.plugin() {
            Some(plugin) => crate::plugin_host::menu(&plugin.identity.id, self.instance),
            None => Vec::new(),
        }
    }

    pub(crate) fn pick_menu(&self, id: String) {
        if let Some(plugin) = self.plugin() {
            crate::plugin_host::menu_pick(&plugin.identity.id, self.instance, id);
        }
    }

    pub(crate) fn set_artifact_states(&self, states: Vec<block_plugin_api::ArtifactState>) {
        let Some(plugin) = self.plugin() else {
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
    editor: Uuid,
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
        editor: Uuid,
        target_id: Uuid,
        target_type: Uuid,
        client_id: Uuid,
    ) -> Self {
        Self {
            plugin,
            editor,
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
            editor: self.editor,
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

    fn settings_region(&mut self, registry: &EditorRegistry) -> HostedRegion {
        self.opened = true;
        HostedRegion {
            editor: RegionEditor {
                plugin: Arc::clone(&self.plugin),
                role: InstanceRole::Artifact(self.editor, self.block),
                instance: self.instance,
                block_types: Arc::clone(registry.plugin_block_types()),
                client_id: self.client_id,
            },
            region: EditorRegion::ArtifactSettings,
            frame: None,
        }
    }

    fn take_draft(&mut self) -> Option<Vec<u8>> {
        crate::plugin_host::artifact_draft(&self.plugin.identity.id, self.instance)
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
