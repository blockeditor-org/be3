use be_sdk_api::{
    DialogAnswer, DialogStatus, EditorCapabilities, EditorInstanceId, FrameSpec, HostPanel,
    InteractionMode, ParentView, ResizeMode, View, ViewId,
};
use be_sdk_manifest::{AppManifest, EditorManifest};
use beui::{Vec2, vec2};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::{cell::RefCell, rc::Rc};
use uuid::Uuid;

use super::{
    ArtifactSession, ArtifactStatus, DirectEditorCapabilities, DirectEditorInteraction,
    DirectEditorResize, EditorRegistry, PendingCreation,
};
use crate::{compositor::ViewEditor, surfaces::HostedView};
use be_host_base::host;
use be_host_runtime::{
    ArtifactSlot, ArtifactState, CreationSlot, CreationState, EditorBlock, Heard, InstanceEvent,
    InstanceRole,
};

/// What one instance's events left for the host object that runs it, until it takes them.
struct Inbox<T> {
    held: Rc<RefCell<Vec<T>>>,
    _mailbox: host::Mailbox,
}

impl<T: 'static> Inbox<T> {
    fn new(
        app_id: &str,
        instance: EditorInstanceId,
        pick: impl Fn(InstanceEvent) -> Option<T> + 'static,
    ) -> Self {
        let held = Rc::new(RefCell::new(Vec::new()));
        let into = Rc::clone(&held);
        let app_id = app_id.to_owned();
        let (mailbox, post) = host::mailbox(move |heard: Heard| {
            if heard.from(&app_id, instance)
                && let Some(value) = pick(heard.event)
            {
                into.borrow_mut().push(value);
            }
        });
        be_host_runtime::listen(post);
        Self {
            held,
            _mailbox: mailbox,
        }
    }

    /// The latest it holds, forgetting the rest.
    fn take_latest(&self) -> Option<T> {
        std::mem::take(&mut *self.held.borrow_mut()).pop()
    }
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

pub(super) struct AppCreation {
    app: Arc<AppManifest>,
    target: CreationTarget,
    instance: EditorInstanceId,
    opened: bool,
    state: CreationState,
    committed: bool,
    dismissed: bool,
    created: Inbox<Result<Uuid, String>>,
}

impl AppCreation {
    pub(super) fn new(app: Arc<AppManifest>, target: CreationTarget) -> Self {
        let instance = next_instance();
        let created = Inbox::new(&app.identity.id, instance, |event| match event {
            InstanceEvent::Created(created) => Some(created),
            _ => None,
        });
        Self {
            app,
            target,
            instance,
            opened: false,
            state: CreationState::Starting,
            committed: false,
            dismissed: false,
            created,
        }
    }

    fn role(&self) -> InstanceRole {
        InstanceRole::Creation(self.target.editor, self.target.template)
    }

    fn hosted(&self, registry: &EditorRegistry, client_id: Uuid) -> HostedView {
        HostedView {
            editor: ViewEditor {
                app: Arc::clone(&self.app),
                role: self.role(),
                instance: self.instance,
                block_types: Arc::clone(registry.app_block_types()),
                client_id,
            },
            region: View::MAIN,
            frame: Some(FrameSpec::default()),
        }
    }
}

pub(crate) struct AppPicker {
    app: Arc<AppManifest>,
    role: InstanceRole,
    instance: EditorInstanceId,
}

impl AppPicker {
    pub(crate) fn new(
        app: Arc<AppManifest>,
        editor: Uuid,
        request: be_host_runtime::PickerRequest,
    ) -> Self {
        let instance = next_instance();
        be_host_runtime::ask_picker(instance, request);
        Self {
            app,
            role: InstanceRole::Picker(editor),
            instance,
        }
    }

    pub(crate) fn rename(app: Arc<AppManifest>, editor: Uuid, block: Uuid, sheet: bool) -> Self {
        Self {
            app,
            role: InstanceRole::Rename(editor, block, sheet),
            instance: next_instance(),
        }
    }

    pub(crate) fn region(&self, registry: &EditorRegistry, client_id: Uuid) -> HostedView {
        HostedView {
            editor: ViewEditor {
                app: Arc::clone(&self.app),
                role: self.role,
                instance: self.instance,
                block_types: Arc::clone(registry.app_block_types()),
                client_id,
            },
            region: View::MAIN,
            frame: Some(FrameSpec::default()),
        }
    }

    pub(crate) fn source(&self) -> InstanceSource {
        InstanceSource {
            app_id: self.app.identity.id.clone(),
            instance: self.instance,
        }
    }

    pub(crate) fn picked(&self) -> Option<be_sdk_api::BlockPick> {
        be_host_runtime::picked(&self.app.identity.id, self.instance)
    }

    pub(crate) fn intrinsic(&self) -> Option<Vec2> {
        be_host_runtime::intrinsic_size(&self.app.identity.id, self.instance)
    }

    pub(crate) fn card(&self) -> bool {
        be_host_runtime::card(&self.app.identity.id, self.instance)
    }

    pub(crate) fn dialog(&self) -> DialogStatus {
        be_host_runtime::dialog_status(&self.app.identity.id, self.instance).unwrap_or_default()
    }
}

impl Drop for AppPicker {
    fn drop(&mut self) {
        be_host_runtime::close(&self.app.identity.id, self.instance);
        be_host_runtime::forget_picker(self.instance);
    }
}

impl Drop for AppCreation {
    fn drop(&mut self) {
        if self.opened {
            be_host_runtime::close(&self.app.identity.id, self.instance);
        }
    }
}

impl PendingCreation for AppCreation {
    fn region(&self, registry: &EditorRegistry, client_id: Uuid) -> Option<HostedView> {
        self.target.dialog.then(|| self.hosted(registry, client_id))
    }

    fn source(&self) -> InstanceSource {
        InstanceSource {
            app_id: self.app.identity.id.clone(),
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
        self.state = be_host_runtime::creation(CreationSlot {
            app: &self.app,
            block_types: registry.app_block_types(),
            client_id,
            instance: self.instance,
            role: self.role(),
        });
    }

    fn dialog(&self) -> DialogStatus {
        let mut status = be_host_runtime::dialog_status(&self.app.identity.id, self.instance)
            .unwrap_or_default();
        if self.dismissed && status.answer.is_none() {
            status.answer = Some(DialogAnswer::Dismissed);
        }
        status
    }

    fn dismiss(&mut self) {
        match be_host_runtime::dialog_status(&self.app.identity.id, self.instance) {
            Some(status) if status.handles_dismiss => {
                be_host_runtime::dismiss(&self.app.identity.id, self.instance);
            }
            _ => self.dismissed = true,
        }
    }

    fn height(&self) -> Option<f32> {
        self.target.dialog.then(|| {
            be_host_runtime::view_size(&self.app.identity.id, self.instance, ViewId::MAIN)
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
            be_host_runtime::commit_creation(&self.app.identity.id, self.instance);
        }
        match self.created.take_latest() {
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

pub(crate) struct AppEditor {
    app: Option<(Arc<AppManifest>, Uuid)>,
    id: Uuid,
    block_type: Uuid,
    view_block: Option<Uuid>,
    instance: EditorInstanceId,
    opened: bool,
    presence_active: bool,
    shown: u32,
}

#[derive(Clone)]
pub(crate) struct InstanceSource {
    pub(crate) app_id: String,
    pub(crate) instance: EditorInstanceId,
}

impl AppEditor {
    pub(super) fn new(app: Arc<AppManifest>, editor: Uuid, id: Uuid, block_type: Uuid) -> Self {
        Self::open(Some((app, editor)), id, block_type)
    }

    pub(super) fn unsupported(id: Uuid, block_type: Uuid) -> Self {
        Self::open(None, id, block_type)
    }

    fn open(app: Option<(Arc<AppManifest>, Uuid)>, id: Uuid, block_type: Uuid) -> Self {
        Self {
            app,
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
        let (app, editor) = self.app.as_ref()?;
        app.editor(editor.into_bytes())
    }

    fn capabilities(&self) -> EditorCapabilities {
        self.manifest()
            .map_or_else(EditorCapabilities::default, |editor| editor.capabilities)
    }

    fn sync_active_presence(&mut self, active: bool) {
        if self.app.is_none() || active == self.presence_active {
            return;
        }
        self.presence_active = active;
        if let Some(app) = self.app() {
            be_host_runtime::set_presence_visible(&app.identity.id, self.instance, active);
        }
    }

    fn presenting(&self) -> bool {
        self.app()
            .is_some_and(|app| be_host_runtime::presenting(&app.identity.id, self.instance))
    }

    fn stop_presenting(&mut self) {
        let Some(app) = self.app() else {
            return;
        };
        if !self.opened {
            return;
        }
        if self.presenting() {
            be_host_runtime::present(&app.identity.id, self.instance, false);
            host::request_repaint();
        }
    }

    pub(crate) fn source(&self) -> Option<InstanceSource> {
        let app = self.app()?;
        Some(InstanceSource {
            app_id: app.identity.id.clone(),
            instance: self.instance,
        })
    }

    fn close(&mut self) {
        self.opened = false;
        if let Some(app) = self.app() {
            be_host_runtime::close(&app.identity.id, self.instance);
        }
    }

    pub(crate) fn app(&self) -> Option<&Arc<AppManifest>> {
        self.app.as_ref().map(|(app, _)| app)
    }

    pub(crate) fn editor_id(&self) -> Option<Uuid> {
        self.app.as_ref().map(|(_, editor)| *editor)
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

    pub(crate) fn accepts(&self, request: be_sdk_manifest::ShellRequest) -> bool {
        self.manifest()
            .is_some_and(|editor| editor.accepts(request))
    }

    pub(crate) fn has_view(&self, view: ParentView) -> bool {
        self.manifest()
            .is_some_and(|editor| editor.views.contains(&view))
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
        let Some(app) = self.app() else {
            return self.change_child(replace);
        };
        if !be_host_stack::discovery::block_type(self.block_type)
            .is_some_and(|block_type| block_type.children.replace)
        {
            return None;
        }
        match be_host_runtime::replace_child(&app.identity.id, self.instance, old, new) {
            Some(true) => Some(true),
            Some(false) => self.change_child(replace),
            None => None,
        }
    }

    pub(crate) fn reorder_child(&self, child: Uuid, after: Option<Uuid>) -> Option<bool> {
        if !be_host_stack::discovery::block_type(self.block_type)?
            .children
            .reorder
        {
            return Some(false);
        }
        self.change_child(be_block::ChildChange::Reorder {
            block: child,
            after,
        })
    }

    fn change_child(&self, change: be_block::ChildChange) -> Option<bool> {
        be_host_stack::change_child(self.id, self.block_type, change)
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
        let Some(app) = self.app() else {
            return Some(UNSUPPORTED_EDITOR_SIZE);
        };
        Some(
            be_host_runtime::intrinsic_size(&app.identity.id, self.instance)
                .unwrap_or_else(|| vec2(420.0, 240.0)),
        )
    }

    pub(crate) fn show_block(&self, open: be_host_runtime::BlockOpen) {
        let Some(app) = self.app() else {
            return;
        };
        be_host_runtime::show_block(app, self.instance, open);
    }

    pub(crate) fn show_dialog(&self, id: Uuid, dialog: be_sdk_api::ShellDialog) {
        if let Some(app) = self.app() {
            be_host_runtime::show_dialog(&app.identity.id, self.instance, id, dialog);
        }
    }

    pub(crate) fn show_history(&self, block: Uuid, commit: Option<[u8; 32]>) {
        if let Some(app) = self.app() {
            be_host_runtime::show_history(&app.identity.id, self.instance, block, commit);
        }
    }

    pub(crate) fn show_panel(&self, panel: HostPanel) {
        if let Some(app) = self.app() {
            be_host_runtime::show_panel(&app.identity.id, self.instance, panel);
        }
    }

    pub(crate) fn claim_shell(&self) {
        be_host_runtime::set_shell(
            self.app()
                .map(|app| (app.identity.id.as_str(), self.instance)),
        );
    }



    pub(crate) fn menu(&self) -> Vec<be_sdk_api::MenuEntry> {
        match self.app() {
            Some(app) => be_host_runtime::menu(&app.identity.id, self.instance),
            None => Vec::new(),
        }
    }

    pub(crate) fn pick_menu(&self, id: String, anchor: Option<beui::Rect>) {
        if let Some(app) = self.app() {
            be_host_runtime::menu_pick(&app.identity.id, self.instance, id, anchor);
        }
    }

    pub(crate) fn set_artifact_states(&self, states: Vec<be_sdk_api::ArtifactState>) {
        let Some(app) = self.app() else {
            return;
        };
        be_host_runtime::set_artifact_states(&app.identity.id, self.instance, states);
    }

    pub(crate) fn tab_closed(&mut self) {
        self.sync_active_presence(false);
        self.stop_presenting();
        self.close();
    }
}

impl Drop for AppEditor {
    fn drop(&mut self) {
        self.stop_presenting();
        self.close();
    }
}

pub(super) struct AppArtifact {
    app: Arc<AppManifest>,
    editor: Uuid,
    block: EditorBlock,
    source: Uuid,
    client_id: Uuid,
    instance: EditorInstanceId,
    opened: bool,
    resync: bool,
    dismissed: bool,
    outcome: Option<Result<(), String>>,
    outcomes: Inbox<Result<(), String>>,
    regenerating: bool,
}

impl AppArtifact {
    pub(super) fn new(
        app: Arc<AppManifest>,
        editor: Uuid,
        source: Uuid,
        target_id: Uuid,
        target_type: Uuid,
        client_id: Uuid,
    ) -> Self {
        let instance = next_instance();
        let outcomes = Inbox::new(&app.identity.id, instance, |event| match event {
            InstanceEvent::ArtifactOutcome(outcome) => Some(outcome),
            _ => None,
        });
        Self {
            app,
            editor,
            source,
            client_id,
            block: EditorBlock {
                id: target_id,
                block_type: target_type,
                view_block: None,
            },
            instance,
            opened: false,
            resync: false,
            dismissed: false,
            outcome: None,
            outcomes,
            regenerating: false,
        }
    }
}

impl Drop for AppArtifact {
    fn drop(&mut self) {
        if self.opened {
            be_host_runtime::close(&self.app.identity.id, self.instance);
        }
    }
}

impl ArtifactSession for AppArtifact {
    fn poll(&mut self, registry: &EditorRegistry, data: &[u8]) -> ArtifactStatus {
        self.opened = true;
        let state = be_host_runtime::artifact(ArtifactSlot {
            app: &self.app,
            block_types: registry.app_block_types(),
            client_id: self.client_id,
            instance: self.instance,
            editor: self.editor,
            block: self.block,
            source: self.source,
            data,
            resync: std::mem::take(&mut self.resync),
        });
        if let Some(outcome) = self.outcomes.take_latest() {
            self.regenerating = false;
            self.outcome = Some(outcome);
        }
        match state {
            ArtifactState::Starting => ArtifactStatus::Starting,
            ArtifactState::Described(summary) => ArtifactStatus::Described(summary),
            ArtifactState::Failed(error) => ArtifactStatus::Failed(error),
        }
    }

    fn settings_region(&mut self, registry: &EditorRegistry) -> HostedView {
        self.opened = true;
        HostedView {
            editor: ViewEditor {
                app: Arc::clone(&self.app),
                role: InstanceRole::Artifact(self.editor, self.block, self.source),
                instance: self.instance,
                block_types: Arc::clone(registry.app_block_types()),
                client_id: self.client_id,
            },
            region: View::ARTIFACT_SETTINGS,
            frame: None,
        }
    }

    fn take_draft(&mut self) -> Option<Vec<u8>> {
        be_host_runtime::artifact_draft(&self.app.identity.id, self.instance)
    }

    fn settings_height(&self) -> f32 {
        be_host_runtime::view_size(
            &self.app.identity.id,
            self.instance,
            ViewId::ARTIFACT_SETTINGS,
        )
        .map_or(ARTIFACT_SETTINGS_HEIGHT, |size| size.y.max(1.0))
    }

    fn cancel_settings(&mut self) {
        self.resync = true;
        self.close_dialog();
    }

    fn dialog(&self) -> DialogStatus {
        let mut status = be_host_runtime::dialog_status(&self.app.identity.id, self.instance)
            .unwrap_or_default();
        if self.dismissed && status.answer.is_none() {
            status.answer = Some(DialogAnswer::Dismissed);
        }
        status
    }

    fn dismiss(&mut self) {
        match be_host_runtime::dialog_status(&self.app.identity.id, self.instance) {
            Some(status) if status.handles_dismiss => {
                be_host_runtime::dismiss(&self.app.identity.id, self.instance);
            }
            _ => self.dismissed = true,
        }
    }

    fn close_dialog(&mut self) {
        self.dismissed = false;
        be_host_runtime::forget_dialog_answer(&self.app.identity.id, self.instance);
    }

    fn regenerate(&mut self, data: &[u8]) {
        self.outcome = None;
        self.regenerating = true;
        be_host_runtime::regenerate_artifact(&self.app.identity.id, self.instance, data);
    }

    fn take_outcome(&mut self) -> Option<Result<(), String>> {
        self.outcome.take()
    }

    fn regenerating(&self) -> bool {
        self.regenerating
    }
}

const ARTIFACT_SETTINGS_HEIGHT: f32 = 72.0;
