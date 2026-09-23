pub(crate) mod plugin;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use beui::{CursorIcon, Key, Pos2, Rect, Vec2, vec2};
use block::BlockAccess;
use block_client::{BlockClient, blocks};
use block_plugin_api::PluginManifest;
use uuid::Uuid;

pub(crate) use crate::block_label::BlockLabel;
use crate::host::{self, HostItem, Ui};

pub(crate) use self::plugin::PluginEditor;

const DIRECT_EDITOR_MIN_ZOOM: f32 = 1.0 / 64.0;
const DIRECT_EDITOR_MAX_ZOOM: f32 = 32.0;
pub struct FocusReport {
    pub block: Option<(Uuid, Uuid)>,
    pub via: Vec<Uuid>,
}

pub enum EditorAction {
    OpenBlock {
        id: Uuid,
        block_type: Uuid,
        via: Option<Uuid>,
        from: Option<Uuid>,
    },
    DragBlock {
        id: Uuid,
        block_type: Uuid,
    },
    Command {
        id: Uuid,
        command: block_plugin_api::BlockCommand,
    },
}

pub struct BlockRenderContext {
    pub corners: [Pos2; 4],
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct DirectEditorCapabilities {
    pub allow_rotation: bool,
    pub preserve_aspect_ratio: bool,
    pub supports_pan_and_zoom: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectEditorInteraction {
    Preview,
    Live,
    Playback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectEditorResize {
    None,
    Horizontal,
    Vertical,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectEditorViewportInput {
    Background,
    Viewport,
    Editor,
}

#[derive(Clone, Copy, Debug)]
pub enum DirectEditorViewportCommand {
    Pan(Vec2),
    Zoom { factor: f32, anchor: Option<Pos2> },
    Fit,
    AutoFit(Uuid),
    ResumeAutoFit,
}

pub struct DirectEditorViewport {
    commands: Vec<DirectEditorViewportCommand>,
    content_rect: Option<Rect>,
    scale: f32,
    gestures_read: bool,
}

impl DirectEditorViewport {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            content_rect: None,
            scale: 1.0,
            gestures_read: false,
        }
    }

    pub fn pan(&mut self, delta: Vec2) {
        self.commands.push(DirectEditorViewportCommand::Pan(delta));
    }

    pub fn change_zoom(&mut self, factor: f32, anchor: Option<Pos2>) {
        self.commands
            .push(DirectEditorViewportCommand::Zoom { factor, anchor });
    }

    pub fn fit(&mut self) {
        self.commands.push(DirectEditorViewportCommand::Fit);
    }

    pub fn resume_auto_fit(&mut self) {
        self.commands
            .push(DirectEditorViewportCommand::ResumeAutoFit);
    }

    pub fn auto_fit(&mut self, target: Uuid) {
        self.commands
            .push(DirectEditorViewportCommand::AutoFit(target));
    }

    pub fn drain(&mut self) -> impl Iterator<Item = DirectEditorViewportCommand> + '_ {
        self.commands.drain(..)
    }

    pub fn content_rect(&self) -> Option<Rect> {
        self.content_rect
    }

    pub fn replace_content_rect(&mut self, rect: Option<Rect>) -> Option<Rect> {
        std::mem::replace(&mut self.content_rect, rect)
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn replace_scale(&mut self, scale: f32) -> f32 {
        std::mem::replace(&mut self.scale, scale)
    }

    pub fn gestures_read(&self) -> bool {
        self.gestures_read
    }

    pub fn set_gestures_read(&mut self, read: bool) {
        self.gestures_read = read;
    }
}

pub fn editor_access_ceiling(client: &BlockClient, id: Uuid) -> BlockAccess {
    let access = client.block_access(id);
    if client.is_dynamic_artifact(id) {
        access.min(BlockAccess::View)
    } else {
        access
    }
}

fn no_access_notice(ui: &mut Ui, id: Uuid) {
    let rect = ui.rect();
    ui.item(
        ("no-access", id),
        rect,
        HostItem::Notice {
            text: "No access".to_owned(),
            spinner: false,
        },
    );
}

fn rect_corners(rect: Rect) -> [Pos2; 4] {
    [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
}

fn paint_block_fallback(
    ui: &mut Ui,
    key: impl Hash,
    rect: Rect,
    block_id: Uuid,
    editors: &EditorAccess<'_>,
) {
    let label = editors
        .client
        .cached_block(block_id)
        .map(|cached| BlockLabel::for_cached(editors.registry(), &cached));
    let (name, automatic) = label
        .as_ref()
        .map_or(("Loading…".to_owned(), false), |label| {
            (label.name.clone(), label.automatic)
        });
    ui.item(
        ("fallback", key),
        rect,
        HostItem::Fallback {
            name,
            automatic,
            icon: label.and_then(|label| label.icon).map(str::to_owned),
        },
    );
}

pub struct EditorAccess<'a> {
    active: Vec<Uuid>,
    access: BlockAccess,
    client: &'a Arc<BlockClient>,
    client_id: Uuid,
    registry: &'a EditorRegistry,
    editors: &'a mut HashMap<Uuid, PluginEditor>,
    simulated: &'a HashMap<Uuid, BlockAccess>,
}

pub fn embedded_editor_ui(
    ui: &mut Ui,
    editors: &mut EditorAccess<'_>,
    block_id: Uuid,
    rect: Rect,
    clip_rect: Rect,
    viewport: &mut DirectEditorViewport,
) -> Option<EditorAction> {
    let input = editors.direct_editor_viewport_input(block_id);
    let previous = viewport.replace_content_rect(Some(rect));
    let previous_scale = viewport.replace_scale(embedded_scale(editors, block_id, rect));
    let action = {
        let mut child = ui.child(rect, clip_rect);
        editors.embedded_direct_editor_ui(block_id, &mut child, viewport)
    };
    viewport.replace_content_rect(previous);
    viewport.replace_scale(previous_scale);
    if input == DirectEditorViewportInput::Viewport && !viewport.gestures_read() {
        viewport_gesture_input(rect.intersect(clip_rect), None, viewport);
    }
    action
}

fn embedded_scale(editors: &mut EditorAccess<'_>, block_id: Uuid, rect: Rect) -> f32 {
    editors
        .direct_editor_intrinsic_size(block_id)
        .filter(|intrinsic| intrinsic.x > 0.0 && intrinsic.y > 0.0)
        .map_or(1.0, |intrinsic| {
            (rect.width() / intrinsic.x).min(rect.height() / intrinsic.y)
        })
}

impl<'a> EditorAccess<'a> {
    pub fn new(
        active: Uuid,
        access: BlockAccess,
        client: &'a Arc<BlockClient>,
        client_id: Uuid,
        registry: &'a EditorRegistry,
        editors: &'a mut HashMap<Uuid, PluginEditor>,
        simulated: &'a HashMap<Uuid, BlockAccess>,
    ) -> Self {
        Self {
            active: vec![active],
            access,
            client,
            client_id,
            registry,
            editors,
            simulated,
        }
    }

    pub fn access(&self) -> BlockAccess {
        self.access
    }

    fn access_for(&self, id: Uuid) -> BlockAccess {
        let simulated = self
            .simulated
            .get(&id)
            .copied()
            .unwrap_or(BlockAccess::Edit);
        self.access
            .min(editor_access_ceiling(self.client, id))
            .min(simulated)
    }

    pub fn client(&self) -> &BlockClient {
        self.client
    }

    pub fn client_handle(&self) -> Arc<BlockClient> {
        Arc::clone(self.client)
    }

    pub fn client_id(&self) -> Uuid {
        self.client_id
    }

    pub fn registry(&self) -> &EditorRegistry {
        self.registry
    }

    pub fn insert(&mut self, editor: PluginEditor) {
        let id = editor.id();
        assert!(
            !self.active.contains(&id),
            "cannot replace an active editor"
        );
        assert!(
            self.editors.insert(id, editor).is_none(),
            "editor {id} is already open"
        );
    }

    pub fn is_open(&self, id: Uuid) -> bool {
        self.editors.contains_key(&id)
    }

    pub fn ensure(&mut self, id: Uuid, block_type: Uuid) {
        if !self.active.contains(&id) && !self.editors.contains_key(&id) {
            self.editors
                .insert(id, self.registry.open(self.client, id, block_type));
        }
    }

    fn with_editor<T>(
        &mut self,
        id: Uuid,
        callback: impl FnOnce(&mut PluginEditor, &mut Self) -> T,
    ) -> Option<T> {
        let mut editor = self.editors.remove(&id)?;
        let nested = self.access_for(id);
        let access = std::mem::replace(&mut self.access, nested);
        self.active.push(id);
        let result = callback(&mut editor, self);
        assert_eq!(self.active.pop(), Some(id));
        self.access = access;
        self.editors.insert(id, editor);
        Some(result)
    }

    fn with_editor_ui<T>(
        &mut self,
        id: Uuid,
        ui: &mut Ui,
        callback: impl FnOnce(&mut PluginEditor, &mut Self, &mut Ui) -> T,
    ) -> Option<T> {
        let access = self.access_for(id);
        if !access.can_view() {
            no_access_notice(ui, id);
            return None;
        }
        self.with_editor(id, |editor, editors| callback(editor, editors, ui))
    }

    pub fn preview_aspect_ratio(&self, id: Uuid) -> Option<f32> {
        self.editors
            .get(&id)
            .and_then(|editor| editor.render_aspect_ratio())
    }

    pub fn render(&mut self, id: Uuid, ui: &mut Ui, context: BlockRenderContext) -> bool {
        if !self.access_for(id).can_view() {
            return false;
        }
        self.with_editor(id, |editor, editors| editor.render(ui, context, editors))
            .unwrap_or(false)
    }

    pub fn direct_editor_capabilities(&self, id: Uuid) -> Option<DirectEditorCapabilities> {
        self.editors
            .get(&id)
            .map(|editor| editor.direct_editor_capabilities())
    }

    pub fn direct_editor_interaction(&self, id: Uuid) -> Option<DirectEditorInteraction> {
        self.editors
            .get(&id)
            .map(|editor| editor.direct_editor_interaction())
    }

    pub fn direct_editor_resize(&self, id: Uuid) -> Option<DirectEditorResize> {
        self.editors
            .get(&id)
            .map(|editor| editor.direct_editor_resize())
    }

    pub fn direct_editor_viewport_input(&self, id: Uuid) -> DirectEditorViewportInput {
        self.editors
            .get(&id)
            .map(PluginEditor::direct_editor_viewport_input)
            .unwrap_or(DirectEditorViewportInput::Background)
    }

    pub fn direct_editor_intrinsic_size(&mut self, id: Uuid) -> Option<Vec2> {
        self.with_editor(id, |editor, _| editor.direct_editor_intrinsic_size())?
    }

    pub fn set_direct_editor_intrinsic_size(&mut self, id: Uuid, size: Vec2) -> bool {
        if !self.access_for(id).can_edit() {
            return false;
        }
        self.with_editor(id, |editor, _| {
            editor.set_direct_editor_intrinsic_size(size)
        })
        .unwrap_or(false)
    }

    pub fn embedded_direct_editor_ui(
        &mut self,
        id: Uuid,
        ui: &mut Ui,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.with_editor_ui(id, ui, |editor, editors, ui| {
            editor.direct_editor_ui(ui, editors, viewport)
        })?
    }

    pub fn block_label(&self, id: Uuid) -> String {
        self.client
            .cached_block(id)
            .map(|cached| BlockLabel::for_cached(self.registry, &cached).name)
            .unwrap_or_else(|| "Block".to_owned())
    }

    pub fn direct_editor_frame_child(&mut self, id: Uuid) -> Option<Uuid> {
        self.with_editor(id, |editor, _| editor.direct_editor_frame_child())?
    }

    pub fn is_frame_child(&self, id: Uuid) -> bool {
        tab_frame().is_some_and(|tab| tab.stack.contains(&id))
    }

    pub fn clear_direct_editor_frame_child(&mut self, id: Uuid) {
        self.with_editor(id, |editor, _| {
            editor.clear_direct_editor_frame_child();
        });
    }

    fn direct_editor_frame_ui(
        &mut self,
        id: Uuid,
        ui: &mut Ui,
        slot: &FrameSlot,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let mut child = ui.child(slot.frame, slot.clip);
        let access = self.access_for(id);
        if !access.can_view() {
            no_access_notice(&mut child, id);
            return None;
        }
        let (action, exit) = self
            .with_editor(id, |editor, editors| {
                direct_editor_frame_ui(editor, &mut child, editors, slot, Some(viewport))
            })
            .unwrap_or((None, false));
        if exit {
            request_frame_exit();
        }
        action
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarDragSource {
    Root,
    Orphaned,
    Block(Uuid),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Chrome {
    #[default]
    Drawn,
    None,
}

#[derive(Clone)]
pub struct FrameSlot {
    pub frame: Rect,
    pub clip: Rect,
    pub content: Option<Rect>,
    pub chrome: Chrome,
    pub trail: Vec<String>,
}

#[derive(Clone)]
struct TabFrame {
    frame: Rect,
    clip: Rect,
    stack: Vec<Uuid>,
    trail: Vec<String>,
}

thread_local! {
    static TAB_FRAME: RefCell<Option<TabFrame>> = const { RefCell::new(None) };
    static FRAME_EXIT: Cell<bool> = const { Cell::new(false) };
    static VIEWPORTS: RefCell<HashMap<Uuid, DirectEditorTabViewport>> = RefCell::new(HashMap::new());
}

fn tab_frame() -> Option<TabFrame> {
    TAB_FRAME.with(|frame| frame.borrow().clone())
}

fn set_tab_frame(frame: Option<TabFrame>) {
    TAB_FRAME.with(|slot| *slot.borrow_mut() = frame);
}

fn request_frame_exit() {
    FRAME_EXIT.with(|exit| exit.set(true));
    host::request_repaint();
}

fn take_frame_exit() -> bool {
    FRAME_EXIT.with(|exit| exit.replace(false))
}

pub fn direct_editor_tab_ui(
    editor: &mut PluginEditor,
    ui: &mut Ui,
    editors: &mut EditorAccess<'_>,
) -> Option<EditorAction> {
    let frame = ui.rect();
    let clip = frame.intersect(ui.clip());
    let mut stack = Vec::new();
    let mut trail = vec![editors.block_label(editor.id())];
    let mut child = editor.direct_editor_frame_child();
    while let Some(id) = child {
        if stack.contains(&id) {
            break;
        }
        stack.push(id);
        trail.push(editors.block_label(id));
        child = editors.direct_editor_frame_child(id);
    }
    let owner = stack.last().copied();
    set_tab_frame(Some(TabFrame {
        frame,
        clip,
        stack: stack.clone(),
        trail,
    }));
    let slot = FrameSlot {
        frame,
        clip,
        content: None,
        chrome: match owner {
            Some(_) => Chrome::None,
            None => Chrome::Drawn,
        },
        trail: Vec::new(),
    };
    let (action, own_exit) = direct_editor_frame_ui(editor, ui, editors, &slot, None);
    let exit = own_exit || take_frame_exit();
    if exit {
        match stack.len() {
            0 | 1 => editor.clear_direct_editor_frame_child(),
            depth => {
                let parent = stack[depth - 2];
                editors.clear_direct_editor_frame_child(parent);
            }
        }
        host::request_repaint();
    }
    action
}

pub fn own_frame_child_ui(
    ui: &mut Ui,
    editors: &mut EditorAccess<'_>,
    block_id: Uuid,
    frame: Rect,
    clip_rect: Rect,
    viewport: &mut DirectEditorViewport,
) -> Option<EditorAction> {
    let clip = frame.intersect(clip_rect);
    let mut stack = Vec::new();
    let mut trail = vec![editors.block_label(block_id)];
    let mut child = editors.direct_editor_frame_child(block_id);
    while let Some(id) = child {
        if id == block_id || stack.contains(&id) {
            break;
        }
        stack.push(id);
        trail.push(editors.block_label(id));
        child = editors.direct_editor_frame_child(id);
    }
    let owner = stack.last().copied();
    let outer_frame = tab_frame();
    let outer_exit = take_frame_exit();
    set_tab_frame(Some(TabFrame {
        frame,
        clip,
        stack: stack.clone(),
        trail,
    }));
    let slot = FrameSlot {
        frame,
        clip,
        content: None,
        chrome: match owner {
            Some(_) => Chrome::None,
            None => Chrome::Drawn,
        },
        trail: Vec::new(),
    };
    let action = editors.direct_editor_frame_ui(block_id, ui, &slot, viewport);
    if take_frame_exit() {
        match stack.len() {
            0 => editors.clear_direct_editor_frame_child(block_id),
            depth => {
                let parent = match depth {
                    1 => block_id,
                    _ => stack[depth - 2],
                };
                editors.clear_direct_editor_frame_child(parent);
            }
        }
        host::request_repaint();
    }
    if outer_exit {
        request_frame_exit();
    }
    set_tab_frame(outer_frame);
    action
}

pub fn frame_child_ui(
    ui: &mut Ui,
    editors: &mut EditorAccess<'_>,
    block_id: Uuid,
    content: Rect,
    clip_rect: Rect,
    viewport: &mut DirectEditorViewport,
) -> Option<EditorAction> {
    let tab = tab_frame()?;
    let depth = tab.stack.iter().position(|id| *id == block_id)?;
    let slot = FrameSlot {
        frame: tab.frame,
        clip: tab.clip,
        content: Some(content.intersect(clip_rect)),
        chrome: match depth + 1 == tab.stack.len() {
            true => Chrome::Drawn,
            false => Chrome::None,
        },
        trail: tab.trail[..depth + 2].to_vec(),
    };
    let previous = viewport.replace_content_rect(Some(content));
    let previous_scale = viewport.replace_scale(embedded_scale(editors, block_id, content));
    let action = editors.direct_editor_frame_ui(block_id, ui, &slot, viewport);
    viewport.replace_content_rect(previous);
    viewport.replace_scale(previous_scale);
    action
}

pub(crate) fn direct_editor_frame_ui(
    editor: &mut PluginEditor,
    ui: &mut Ui,
    editors: &mut EditorAccess<'_>,
    slot: &FrameSlot,
    outer: Option<&mut DirectEditorViewport>,
) -> (Option<EditorAction>, bool) {
    let id = editor.id();
    let read_only = !editors.access().can_edit();
    let viewport_state = VIEWPORTS
        .with(|viewports| viewports.borrow().get(&id).copied())
        .unwrap_or_default();
    let owns_frame = editor.direct_editor_owns_frame();
    let mut bands = DirectEditorTabBands {
        id,
        owns_frame,
        slot: slot.clone(),
        capabilities: editor.direct_editor_capabilities(),
        read_only,
        viewport: DirectEditorViewport::new(),
        viewport_state,
        editor,
        editors,
        outer,
        exit: false,
        action: None,
    };
    let band = match owns_frame {
        true => slot.frame,
        false => slot
            .content
            .map_or(slot.frame, |content| content.intersect(slot.frame)),
    };
    let mut host = ui.child(band, slot.clip);
    bands.content_ui(&mut host);
    (bands.action, bands.exit)
}

struct DirectEditorTabBands<'a, 'b> {
    id: Uuid,
    owns_frame: bool,
    slot: FrameSlot,
    exit: bool,
    capabilities: DirectEditorCapabilities,
    read_only: bool,
    viewport: DirectEditorViewport,
    viewport_state: DirectEditorTabViewport,
    editor: &'a mut PluginEditor,
    editors: &'a mut EditorAccess<'b>,
    outer: Option<&'a mut DirectEditorViewport>,
    action: Option<EditorAction>,
}

impl DirectEditorTabBands<'_, '_> {
    fn outer_is_placed(&self) -> bool {
        self.outer
            .as_deref()
            .is_some_and(|outer| outer.content_rect().is_some())
    }

    fn draw(&mut self, ui: &mut Ui) -> Option<EditorAction> {
        let owns_frame = self.owns_frame;
        let slot = self.slot.clone();
        let placed = self.outer_is_placed();
        let viewport = if placed {
            self.outer
                .as_deref_mut()
                .expect("outer_is_placed implies outer is Some")
        } else {
            &mut self.viewport
        };
        let editor = &mut *self.editor;
        let editors = &mut *self.editors;
        let action = match owns_frame {
            true => editor.direct_editor_frame_ui(ui, editors, &slot, viewport),
            false => editor.direct_editor_ui(ui, editors, viewport),
        };
        if owns_frame {
            self.exit |= self.editor.take_direct_editor_frame_exit();
        }
        action
    }

    fn child_content_ui(&mut self, ui: &mut Ui) {
        let band = ui.rect();
        let rect = match self.owns_frame {
            true => self.slot.frame,
            false => band,
        };
        let action = {
            let mut child = ui.child(rect, rect);
            self.draw(&mut child)
        };
        self.record(action);
        let input = self.editor.direct_editor_viewport_input();
        let viewport = self
            .outer
            .as_deref_mut()
            .expect("child_content_ui is only reached once outer_is_placed()");
        if input == DirectEditorViewportInput::Viewport && !viewport.gestures_read() {
            viewport_gesture_input(band.intersect(ui.clip()), None, viewport);
        }
    }

    fn record(&mut self, action: Option<EditorAction>) {
        if self.action.is_none() {
            self.action = action;
        }
    }

    fn content_ui(&mut self, ui: &mut Ui) {
        if self.outer_is_placed() {
            self.child_content_ui(ui);
            return;
        }
        let id = self.id;
        let band = ui.rect();
        let viewport_size = self
            .editor
            .direct_editor_viewport_rect(band)
            .size()
            .max(Vec2::splat(1.0));
        let intrinsic_size = self
            .editor
            .direct_editor_intrinsic_size()
            .unwrap_or_default();
        let content_size = vec2(
            viewport_size.x.max(intrinsic_size.x),
            viewport_size.y.max(intrinsic_size.y),
        );
        if !self.capabilities.supports_pan_and_zoom {
            let action = self.draw(ui);
            self.record(action);
            return;
        }
        let allocated = band;
        let viewport_rect = self.editor.direct_editor_viewport_rect(allocated);
        if let Some(previous_center) = self.viewport_state.center.replace(viewport_rect.center()) {
            self.viewport_state.pan += previous_center - viewport_rect.center();
        }
        let transformed_size = content_size * self.viewport_state.zoom;
        let content_rect = Rect::from_center_size(
            viewport_rect.center() + self.viewport_state.pan,
            transformed_size,
        );
        self.viewport.replace_content_rect(Some(content_rect));
        self.viewport.replace_scale(self.viewport_state.zoom);
        let fills_viewport = self.editor.direct_editor_fills_viewport();

        let mut viewport_input = self.editor.direct_editor_viewport_input();
        if self.read_only && viewport_input == DirectEditorViewportInput::Editor {
            viewport_input = DirectEditorViewportInput::Background;
        }
        let editor_rect = match (self.owns_frame, fills_viewport) {
            (true, _) => allocated,
            (false, true) => viewport_rect,
            (false, false) => content_rect,
        };
        let action = {
            let mut child = ui.child(editor_rect, editor_rect);
            self.draw(&mut child)
        };
        self.record(action);

        match viewport_input {
            DirectEditorViewportInput::Editor => {}
            DirectEditorViewportInput::Background => viewport_gesture_input(
                viewport_rect,
                (!self.read_only).then_some(content_rect),
                &mut self.viewport,
            ),
            DirectEditorViewportInput::Viewport => {
                viewport_gesture_input(viewport_rect, None, &mut self.viewport)
            }
        }

        let commands: Vec<_> = self.viewport.drain().collect();
        for command in commands {
            match command {
                DirectEditorViewportCommand::Pan(delta) => {
                    self.viewport_state.pan += delta;
                    if let Some(auto_fit) = &mut self.viewport_state.auto_fit {
                        auto_fit.enabled = false;
                    }
                }
                DirectEditorViewportCommand::Zoom { factor, anchor } => {
                    let old_zoom = self.viewport_state.zoom;
                    let new_zoom =
                        (old_zoom * factor).clamp(DIRECT_EDITOR_MIN_ZOOM, DIRECT_EDITOR_MAX_ZOOM);
                    if new_zoom != old_zoom {
                        let anchor = anchor.unwrap_or_else(|| viewport_rect.center());
                        self.viewport_state.pan = (anchor - viewport_rect.center())
                            - ((anchor - viewport_rect.center()) - self.viewport_state.pan)
                                * (new_zoom / old_zoom);
                        self.viewport_state.zoom = new_zoom;
                    }
                    if let Some(auto_fit) = &mut self.viewport_state.auto_fit {
                        auto_fit.enabled = false;
                    }
                }
                DirectEditorViewportCommand::Fit => {
                    fit_direct_editor_viewport(
                        &mut self.viewport_state,
                        viewport_size,
                        content_size,
                    );
                    if let Some(auto_fit) = &mut self.viewport_state.auto_fit {
                        auto_fit.enabled = false;
                    }
                }
                DirectEditorViewportCommand::AutoFit(target) => {
                    let auto_fit = self.viewport_state.auto_fit.get_or_insert(AutoFitState {
                        target,
                        enabled: true,
                    });
                    if auto_fit.target != target {
                        *auto_fit = AutoFitState {
                            target,
                            enabled: true,
                        };
                    }
                    if auto_fit.enabled {
                        fit_direct_editor_viewport(
                            &mut self.viewport_state,
                            viewport_size,
                            content_size,
                        );
                    }
                }
                DirectEditorViewportCommand::ResumeAutoFit => {
                    if let Some(auto_fit) = &mut self.viewport_state.auto_fit {
                        auto_fit.enabled = true;
                        fit_direct_editor_viewport(
                            &mut self.viewport_state,
                            viewport_size,
                            content_size,
                        );
                    }
                }
            }
        }
        let state = self.viewport_state;
        VIEWPORTS.with(|viewports| viewports.borrow_mut().insert(id, state));
    }
}

fn viewport_gesture_input(
    viewport_rect: Rect,
    steered: Option<Rect>,
    viewport: &mut DirectEditorViewport,
) {
    let outside = |position: Pos2| {
        !viewport_rect.contains(position)
            || steered.is_some_and(|rect| rect.contains(position))
            || host::claimed(position)
    };
    if let Some(pinch) = host::input(|input| input.pinch) {
        if !outside(pinch.center) {
            if (pinch.zoom - 1.0).abs() > f32::EPSILON {
                viewport.change_zoom(pinch.zoom, Some(pinch.center));
            }
            if pinch.pan != Vec2::ZERO {
                viewport.pan(pinch.pan);
            }
        }
        return;
    }
    let Some(pointer) = host::pointer().filter(|pointer| !outside(*pointer)) else {
        return;
    };
    let (scroll, zoom_delta, command, panning, delta) = host::input(|input| {
        (
            input.scroll,
            input.zoom,
            input.modifiers.ctrl,
            input.middle_down || (host::key_down(Key::Space) && input.primary_down),
            input.delta,
        )
    });
    if panning {
        host::set_cursor(CursorIcon::Grabbing);
        viewport.pan(delta);
    }
    if (zoom_delta - 1.0).abs() > f32::EPSILON {
        viewport.change_zoom(zoom_delta, Some(pointer));
    } else if command && scroll.y != 0.0 {
        viewport.change_zoom((scroll.y * 0.002).exp(), Some(pointer));
    } else if scroll != Vec2::ZERO {
        viewport.pan(scroll);
    }
}

fn fit_direct_editor_viewport(
    viewport: &mut DirectEditorTabViewport,
    viewport_size: Vec2,
    content_size: Vec2,
) {
    viewport.zoom = (viewport_size.x / content_size.x)
        .min(viewport_size.y / content_size.y)
        .min(1.0)
        .clamp(DIRECT_EDITOR_MIN_ZOOM, DIRECT_EDITOR_MAX_ZOOM);
    viewport.pan = Vec2::ZERO;
}

#[derive(Clone, Copy, Debug)]
struct AutoFitState {
    target: Uuid,
    enabled: bool,
}

#[derive(Clone, Copy, Debug)]
struct DirectEditorTabViewport {
    zoom: f32,
    pan: Vec2,
    center: Option<Pos2>,
    auto_fit: Option<AutoFitState>,
}

impl Default for DirectEditorTabViewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            center: None,
            auto_fit: None,
        }
    }
}

type OpenEditor = Box<dyn Fn(&BlockClient, Uuid) -> PluginEditor>;
type CreateOptions = Box<dyn Fn() -> Box<dyn PendingCreation>>;

struct ArtifactProvider(Arc<PluginManifest>);

pub(super) trait ArtifactSession {
    fn poll(
        &mut self,
        registry: &EditorRegistry,
        client: &Arc<BlockClient>,
        data: &[u8],
    ) -> ArtifactStatus;
    fn settings_ui(
        &mut self,
        ui: &mut Ui,
        registry: &EditorRegistry,
        client: &Arc<BlockClient>,
        draft: &mut Vec<u8>,
    );
    fn settings_height(&self) -> f32;
    fn summary(&self, draft: &[u8]) -> Option<String>;
    fn cancel_settings(&mut self);
    fn regenerate(&mut self, client: &Arc<BlockClient>, data: &[u8]);
    fn take_outcome(&mut self) -> Option<Result<(), String>>;
    fn regenerating(&self) -> bool;
}

pub(super) enum ArtifactStatus {
    Starting,
    Described { source: Uuid, summary: String },
    Failed(String),
}

pub(super) trait PendingCreation {
    fn ui(&mut self, ui: &mut Ui, editors: &mut EditorAccess<'_>) -> CreationStep;
    fn height(&self) -> Option<f32>;
    fn create(&mut self, client: &BlockClient) -> Result<Option<PluginEditor>, String>;
}

#[derive(Clone, Copy)]
pub(super) enum CreationStep {
    Options(bool),
    Working,
}

struct CreateBlock(CreateOptions);

struct EditorRegistration {
    block_type: Uuid,
    display_name: &'static str,
    icon: &'static str,
    create: Option<CreateBlock>,
    open: OpenEditor,
    can_add_child: bool,
    can_delete_child: bool,
    can_replace_child: bool,
    default_important: bool,
    dynamic_artifact: Option<ArtifactProvider>,
}

pub(crate) struct BlockTypeEntry {
    pub(crate) display_name: String,
    pub(crate) icon: &'static str,
    pub(crate) add: bool,
    pub(crate) delete: bool,
    pub(crate) replace: bool,
}

pub struct EditorRegistry {
    registrations: HashMap<Uuid, EditorRegistration>,
    new_block_actions: Vec<(&'static str, Uuid, bool)>,
    plugin_block_types: Arc<Vec<block_plugin_api::BlockTypeDescriptor>>,
}

impl EditorRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            registrations: HashMap::new(),
            new_block_actions: Vec::new(),
            plugin_block_types: Arc::default(),
        };
        for manifest in plugin::discovery::manifests() {
            registry.register_plugin(manifest);
        }
        registry.plugin_block_types =
            Arc::new(plugin::block_type_descriptors(registry.block_types()));
        registry
    }

    fn block_types(&self) -> Vec<(Uuid, BlockTypeEntry)> {
        let mut types: Vec<_> = self
            .registrations
            .values()
            .map(|registration| {
                (
                    registration.block_type,
                    BlockTypeEntry {
                        display_name: registration.display_name.to_owned(),
                        icon: registration.icon,
                        add: registration.can_add_child,
                        delete: registration.can_delete_child,
                        replace: registration.can_replace_child,
                    },
                )
            })
            .collect();
        types.sort_by_key(|(block_type, _)| *block_type);
        types
    }

    pub(super) fn plugin_block_types(&self) -> &Arc<Vec<block_plugin_api::BlockTypeDescriptor>> {
        &self.plugin_block_types
    }

    fn insert(&mut self, registration: EditorRegistration) {
        if registration.create.is_some() {
            self.new_block_actions.push((
                registration.display_name,
                registration.block_type,
                registration.default_important,
            ));
        }
        self.registrations
            .insert(registration.block_type, registration);
    }

    fn register_plugin(&mut self, manifest: Arc<PluginManifest>) {
        let block_type = Uuid::from_bytes(manifest.block_type);
        let display_name: &'static str = Box::leak(manifest.display_name.clone().into_boxed_str());
        let icon: &'static str = Box::leak(manifest.icon.clone().into_boxed_str());
        self.insert(EditorRegistration {
            block_type,
            display_name,
            icon,
            create: match manifest.creation {
                block_plugin_api::CreationMode::Immediate
                | block_plugin_api::CreationMode::Dialog => {
                    let manifest = Arc::clone(&manifest);
                    Some(CreateBlock(Box::new(move || {
                        Box::new(plugin::PluginCreation::new(Arc::clone(&manifest)))
                    })))
                }
                block_plugin_api::CreationMode::None => None,
            },
            open: {
                let manifest = Arc::clone(&manifest);
                Box::new(move |client, id| {
                    let block = blocks::open(client, id, block_type)
                        .expect("a registered plugin block type is in the erased table");
                    PluginEditor::new(Arc::clone(&manifest), block)
                })
            },
            can_add_child: manifest.children.add,
            can_delete_child: manifest.children.delete,
            can_replace_child: manifest.children.replace,
            default_important: manifest.important,
            dynamic_artifact: manifest
                .regions
                .contains(&block_plugin_api::EditorRegion::ArtifactSettings)
                .then(|| ArtifactProvider(Arc::clone(&manifest))),
        });
    }

    pub fn new_block_actions(&self) -> &[(&'static str, Uuid, bool)] {
        &self.new_block_actions
    }

    pub fn display_name(&self, block_type: Uuid) -> Option<&'static str> {
        self.registrations
            .get(&block_type)
            .map(|registration| registration.display_name)
    }

    pub fn icon(&self, block_type: Uuid) -> Option<&'static str> {
        self.registrations
            .get(&block_type)
            .map(|registration| registration.icon)
    }

    pub(super) fn artifact_session(
        &self,
        source_type: Uuid,
        target_id: Uuid,
        target_type: Uuid,
        client_id: Uuid,
    ) -> Result<Box<dyn ArtifactSession>, String> {
        let registration = self
            .registrations
            .get(&source_type)
            .ok_or_else(|| format!("unsupported dynamic artifact source type {source_type}"))?;
        match &registration.dynamic_artifact {
            Some(ArtifactProvider(manifest)) => Ok(Box::new(plugin::PluginArtifact::new(
                Arc::clone(manifest),
                target_id,
                target_type,
                client_id,
            ))),
            None => Err(format!(
                "{} blocks do not generate dynamic artifacts",
                registration.display_name
            )),
        }
    }

    pub(super) fn create(&self, block_type: Uuid) -> Option<Box<dyn PendingCreation>> {
        let CreateBlock(options) = self.registrations.get(&block_type)?.create.as_ref()?;
        Some(options())
    }

    pub fn open(&self, client: &BlockClient, id: Uuid, block_type: Uuid) -> PluginEditor {
        self.registrations.get(&block_type).map_or_else(
            || PluginEditor::unsupported(id, block_type),
            |registration| (registration.open)(client, id),
        )
    }
}
