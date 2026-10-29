use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    rc::Rc,
};

use be_graph::Access;
use beui::native::with_document;
use beui::reactive::*;
use beui::styled::*;
use beui::{
    Color32, Dismiss, Key, KeyEvent, NodeId, Pos2, Rect, ScrollGesture, Vec2, ZoomGesture, vec2,
};
use block_plugin_api::{
    BarAction, ChildId, ChildMode, ChildPreview, CreationProgress, DialogFrame, DialogStatus,
    EditorInstanceId, EditorRegion, FitArea, FrameChrome, FrameSpec, HostPanel, HostWindowId,
    PanMode, TopBar, ViewChange,
};
use uuid::Uuid;

use crate::desktop_host::WindowSurface;

use super::region::{ChildView, PanDrag, PluginRegion, RegionEditor};
use crate::editors::plugin::{InstanceSource, PluginPicker};
use crate::editors::{
    Chrome, EditorRegistry, PendingCreation, PluginEditor, editor_access_ceiling,
};
use crate::surfaces::HostItem;
use crate::plugin_host::{self, ChildCommit, EditorView, HostChild, HostChildStatus, HostContent};
use crate::surfaces::{
    ConflictNotice, HostItemFace, HostedRegion, HostedSurface, StaleBlock, SurfaceId,
};
use crate::ui::HostPanelSurface;
use beui::{Align, Direction, ItemSize, Placement};

const MIN_ZOOM: f32 = 1.0 / 64.0;
const MAX_ZOOM: f32 = 32.0;
const CHILD_UNAVAILABLE: &str = "the block is already open above this editor";
const NO_PICKER: &str = "No editor can pick blocks here.";

pub(crate) struct State {
    registry: RefCell<Rc<EditorRegistry>>,
    client_id: Cell<Uuid>,
    open: RefCell<HashMap<Uuid, PluginEditor>>,
    parents: RefCell<HashMap<Uuid, Uuid>>,
    simulated: RefCell<HashMap<Uuid, Access>>,
    creations: RefCell<HashMap<CreationKey, CreationChild>>,
    pickers: RefCell<HashMap<CreationKey, PluginPicker>>,
    commits: RefCell<HashMap<(Uuid, ChildId), ChildCommit>>,
    settings_children: RefCell<HashMap<(Uuid, ChildId), Uuid>>,
    settings_shown: RefCell<HashMap<CreationKey, Uuid>>,
    settings_status: RefCell<Option<(Uuid, f32, DialogStatus)>>,
}

pub(crate) type CreationKey = (Uuid, EditorRegion, ChildId);

struct CreationChild {
    creation: Box<dyn PendingCreation>,
    commit: Option<ChildCommit>,
    progress: Option<CreationProgress>,
}

#[derive(Clone, Copy)]
pub(crate) struct Editors(Stored<State>);

pub(crate) struct OwnedEditors {
    _state: OwnedStored<State>,
    editors: Editors,
}

impl std::ops::Deref for OwnedEditors {
    type Target = Editors;

    fn deref(&self) -> &Editors {
        &self.editors
    }
}

thread_local! {
    static EDITORS: Cell<Option<Editors>> = const { Cell::new(None) };
}

pub(crate) fn release_revisions(ids: &[Uuid]) {
    if let Some(editors) = EDITORS.get() {
        let removed = editors.with(|open| {
            ids.iter()
                .filter_map(|id| open.remove(id))
                .collect::<Vec<_>>()
        });
        for id in ids {
            editors.forget_parent(*id);
        }
        drop(removed);
        super::changed();
    }
}

pub(crate) fn editors() -> Editors {
    EDITORS
        .get()
        .filter(|editors| editors.0.is_alive())
        .expect("the editors are installed before the document is built")
}

impl OwnedEditors {
    pub(crate) fn install(registry: Rc<EditorRegistry>, client_id: Uuid) -> Self {
        let state = OwnedStored::new(State {
            registry: RefCell::new(registry),
            client_id: Cell::new(client_id),
            open: RefCell::new(HashMap::new()),
            parents: RefCell::new(HashMap::new()),
            simulated: RefCell::new(HashMap::new()),
            creations: RefCell::new(HashMap::new()),
            pickers: RefCell::new(HashMap::new()),
            commits: RefCell::new(HashMap::new()),
            settings_children: RefCell::new(HashMap::new()),
            settings_shown: RefCell::new(HashMap::new()),
            settings_status: RefCell::new(None),
        });
        let editors = Editors(*state);
        EDITORS.set(Some(editors));
        Self {
            _state: state,
            editors,
        }
    }
}

impl Editors {
    pub(crate) fn reset(&self, registry: Rc<EditorRegistry>) {
        let state = self.0.value();
        *state.registry.borrow_mut() = registry;
        let closed: Vec<PluginEditor> =
            self.with(|open| open.drain().map(|(_, editor)| editor).collect());
        drop(closed);
        state.parents.borrow_mut().clear();
        state.creations.borrow_mut().clear();
        state.commits.borrow_mut().clear();
        state.settings_children.borrow_mut().clear();
        state.settings_shown.borrow_mut().clear();
        state.simulated.borrow_mut().clear();
        super::changed();
    }

    pub(crate) fn simulated(&self, id: Uuid) -> Option<Access> {
        let state = self.0.value();
        state.simulated.borrow().get(&id).copied()
    }

    pub(crate) fn registry(&self) -> Rc<EditorRegistry> {
        let state = self.0.value();
        Rc::clone(&state.registry.borrow())
    }

    pub(crate) fn client_id(&self) -> Uuid {
        let state = self.0.value();
        state.client_id.get()
    }

    pub(crate) fn with<R>(&self, act: impl FnOnce(&mut HashMap<Uuid, PluginEditor>) -> R) -> R {
        let state = self.0.value();
        act(&mut state.open.borrow_mut())
    }

    pub(crate) fn with_simulated<R>(&self, act: impl FnOnce(&mut HashMap<Uuid, Access>) -> R) -> R {
        let state = self.0.value();
        let result = act(&mut state.simulated.borrow_mut());
        super::changed();
        result
    }

    fn access(&self, id: Uuid, above: Access) -> Access {
        let state = self.0.value();
        let simulated = state
            .simulated
            .borrow()
            .get(&id)
            .copied()
            .unwrap_or(Access::Manage);
        above.min(editor_access_ceiling(id)).min(simulated)
    }

    pub(crate) fn parent_of(&self, id: Uuid) -> Option<Uuid> {
        let state = self.0.value();
        state.parents.borrow().get(&id).copied()
    }

    pub(crate) fn forget_parent(&self, id: Uuid) {
        let state = self.0.value();

        state.parents.borrow_mut().remove(&id);
    }

    pub(crate) fn block_of(&self, plugin_id: &str, instance: EditorInstanceId) -> Option<Uuid> {
        self.with(|open| {
            open.values()
                .find(|editor| {
                    editor.instance() == instance
                        && editor
                            .plugin()
                            .is_some_and(|plugin| plugin.identity.id == plugin_id)
                })
                .map(PluginEditor::id)
        })
    }

    fn ensure(
        &self,
        id: Uuid,
        block_type: Uuid,
        view_block: Option<Uuid>,
        editor: Option<Uuid>,
        parent: Uuid,
    ) {
        let state = self.0.value();
        if parent != id {
            state.parents.borrow_mut().insert(id, parent);
        }
        let registry = self.registry();
        let opened = crate::editors::Opened {
            id,
            block_type,
            view_block,
        };
        let wanted = editor.filter(|editor| registry.edits(*editor, opened));
        let replaced = self.with(|open| {
            let kept = open.get(&id).is_some_and(|existing| {
                wanted.is_none_or(|wanted| existing.editor_id() == Some(wanted))
            });
            if kept {
                return None;
            }
            open.insert(id, registry.open_with(opened, wanted))
        });
        drop(replaced);
    }

    fn ensure_creation(&self, key: CreationKey, editor: Uuid, template: &str) -> bool {
        let state = self.0.value();
        if state.creations.borrow().contains_key(&key) {
            return true;
        }
        let Some((_, creation)) = self.registry().create(editor, template) else {
            return false;
        };
        let (parent, _, child) = key;
        let commit = state.commits.borrow_mut().remove(&(parent, child));
        state.creations.borrow_mut().insert(
            key,
            CreationChild {
                creation,
                commit,
                progress: None,
            },
        );
        true
    }

    fn forget_creation(&self, key: &CreationKey) {
        let state = self.0.value();
        let (parent, _, child) = key;
        state.commits.borrow_mut().remove(&(*parent, *child));
        let forgotten = state.creations.borrow_mut().remove(key);
        drop(forgotten);
    }

    fn creation_region(&self, key: &CreationKey) -> Option<HostedRegion> {
        let state = self.0.value();
        let registry = self.registry();
        state
            .creations
            .borrow()
            .get(key)?
            .creation
            .region(&registry, self.client_id())
    }

    fn creation_status(
        &self,
        key: &CreationKey,
    ) -> Option<(Option<CreationProgress>, Option<f32>, DialogStatus)> {
        let state = self.0.value();
        let creations = state.creations.borrow();
        let child = creations.get(key)?;
        Some((
            child.progress.clone(),
            child.creation.height(),
            child.creation.dialog(),
        ))
    }

    pub(crate) fn dismiss_creations(&self, parent: Uuid, children: &[ChildId]) {
        let state = self.0.value();
        let mut dismissed = false;
        for ((owner, _, child), creation) in state.creations.borrow_mut().iter_mut() {
            if *owner == parent && children.contains(child) {
                creation.creation.dismiss();
                dismissed = true;
            }
        }
        if dismissed {
            super::changed();
        }
    }

    pub(crate) fn commit_creations(&self, parent: Uuid, commits: Vec<ChildCommit>) {
        let state = self.0.value();
        let mut creations = state.creations.borrow_mut();
        for commit in commits {
            let held = creations
                .iter_mut()
                .find(|((owner, _, child), _)| *owner == parent && *child == commit.child);
            match held {
                Some((_, creation)) => creation.commit = Some(commit),
                None => {
                    state
                        .commits
                        .borrow_mut()
                        .insert((parent, commit.child), commit);
                }
            }
        }
    }

    fn show_settings(&self, key: CreationKey, block: Uuid) {
        let state = self.0.value();
        let (parent, _, child) = key;
        state
            .settings_children
            .borrow_mut()
            .insert((parent, child), block);
        state.settings_shown.borrow_mut().insert(key, block);
    }

    fn hide_settings(&self, key: &CreationKey) {
        let state = self.0.value();
        state.settings_shown.borrow_mut().remove(key);
    }

    pub(crate) fn forget_hidden_settings(&self) {
        let state = self.0.value();
        let shown: Vec<(Uuid, ChildId)> = state
            .settings_shown
            .borrow()
            .keys()
            .map(|(parent, _, child)| (*parent, *child))
            .collect();
        state
            .settings_children
            .borrow_mut()
            .retain(|key, _| shown.contains(key));
    }

    pub(crate) fn requested_settings(&self) -> Option<Uuid> {
        let state = self.0.value();
        state.settings_shown.borrow().values().next().copied()
    }

    pub(crate) fn settings_block(&self, parent: Uuid, child: ChildId) -> Option<Uuid> {
        let state = self.0.value();
        state
            .settings_children
            .borrow()
            .get(&(parent, child))
            .copied()
    }

    pub(crate) fn set_settings_status(&self, status: Option<(Uuid, f32, DialogStatus)>) {
        let state = self.0.value();
        if *state.settings_status.borrow() == status {
            return;
        }
        *state.settings_status.borrow_mut() = status;
        super::changed();
    }

    fn settings_status(&self, block: Uuid) -> Option<(f32, DialogStatus)> {
        let state = self.0.value();
        state
            .settings_status
            .borrow()
            .as_ref()
            .filter(|(shown, _, _)| *shown == block)
            .map(|(_, height, dialog)| (*height, *dialog))
    }

    pub(crate) fn hosted_sources(&self) -> Vec<(Uuid, InstanceSource)> {
        let state = self.0.value();
        let creations = state.creations.borrow();
        let pickers = state.pickers.borrow();
        creations
            .values()
            .map(|child| child.creation.source())
            .chain(pickers.values().map(PluginPicker::source))
            .map(|source| (hosted_parent(source.instance), source))
            .collect()
    }

    fn ensure_picker(&self, key: CreationKey, request: crate::plugin_host::PickerRequest) -> bool {
        let state = self.0.value();
        if state.pickers.borrow().contains_key(&key) {
            return true;
        }
        let Some(picker) = self.registry().picker(request) else {
            return false;
        };
        state.pickers.borrow_mut().insert(key, picker);
        true
    }

    fn forget_picker(&self, key: &CreationKey) {
        let forgotten = self.0.value().pickers.borrow_mut().remove(key);
        drop(forgotten);
    }

    fn picker_region(&self, key: &CreationKey) -> Option<HostedRegion> {
        let state = self.0.value();
        let registry = self.registry();
        let pickers = state.pickers.borrow();
        Some(pickers.get(key)?.region(&registry, self.client_id()))
    }

    fn picker_status(
        &self,
        key: &CreationKey,
    ) -> Option<(Option<block_plugin_api::BlockPick>, Option<Vec2>, bool)> {
        let state = self.0.value();
        let pickers = state.pickers.borrow();
        let picker = pickers.get(key)?;
        Some((picker.picked(), picker.intrinsic(), picker.card()))
    }

    pub(crate) fn step_creations(&self) {
        let state = self.0.value();
        let registry = self.registry();
        let client_id = self.client_id();
        let mut changed = false;
        for child in state.creations.borrow_mut().values_mut() {
            if matches!(
                child.progress,
                Some(CreationProgress::Created(_) | CreationProgress::Failed(_))
            ) {
                continue;
            }
            child.creation.step(&registry, client_id);
            let progress = match &child.commit {
                None => CreationProgress::Options,
                Some(commit) => match child.creation.create() {
                    Ok(Some(block)) => {
                        crate::be::set_parent(block, commit.parent);
                        if let Some(name) = commit.name.clone() {
                            crate::be::name_when_created(block, name);
                        }
                        CreationProgress::Created(block.into_bytes())
                    }
                    Ok(None) => CreationProgress::Working,
                    Err(error) => {
                        child.commit = None;
                        CreationProgress::Failed(error)
                    }
                },
            };
            if child.progress.as_ref() != Some(&progress) {
                child.progress = Some(progress);
                changed = true;
            }
        }
        if changed {
            super::changed();
        }
    }

    fn handle(&self, id: Uuid) -> Option<Handle> {
        self.with(|open| open.get(&id).map(Handle::of))
    }

    fn region(&self, handle: &Handle) -> Option<RegionEditor> {
        Some(RegionEditor {
            plugin: handle.plugin.clone()?,
            role: handle.role,
            instance: handle.instance,
            block_types: self.registry().plugin_block_types().clone(),
            client_id: self.client_id(),
        })
    }
}

#[derive(Clone)]
struct Handle {
    block_type: Uuid,
    plugin: Option<std::sync::Arc<block_plugin_manifest::PluginManifest>>,
    instance: block_plugin_api::EditorInstanceId,
    role: plugin_host::InstanceRole,
    pan_and_zoom: bool,
    max_zoom: Option<u32>,
    pan: PanMode,
    preview: bool,
}

impl Handle {
    fn of(editor: &PluginEditor) -> Self {
        let capabilities = editor.direct_editor_capabilities();
        Self {
            block_type: editor.block_type(),
            plugin: editor.plugin().cloned(),
            instance: editor.instance(),
            role: editor.role(),
            pan_and_zoom: capabilities.supports_pan_and_zoom,
            max_zoom: capabilities.max_zoom,
            pan: capabilities.pan,
            preview: editor.has_region(EditorRegion::Preview),
        }
    }

    fn plugin_id(&self) -> Option<&str> {
        self.plugin
            .as_ref()
            .map(|plugin| plugin.identity.id.as_str())
    }
}

#[derive(Clone, Copy)]
struct Nesting {
    above: Stored<Vec<Uuid>>,
    access: Access,
}

impl Nesting {
    fn root() -> Self {
        Self {
            above: store(Vec::new()),
            access: Access::Manage,
        }
    }

    fn within(&self, id: Uuid, access: Access) -> Self {
        let mut above = self.above.with(Vec::clone);
        above.push(id);
        Self {
            above: store(above),
            access,
        }
    }
}

#[derive(Clone, Copy)]
struct TabContext {
    stack: Memo<Vec<Uuid>>,
    top_bar: Prop<TopBar>,
    portals: Signal<Vec<(Uuid, NodeId)>>,
    exit: ClickCallback,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct ChildReporter {
    bar: Callback<BarAction>,
    focus: Callback<()>,
}

#[derive(Clone, Copy)]
struct FocusedWithin(Callback<()>);

#[derive(Clone, Copy)]
pub(crate) struct ChildReports {
    statuses: Stored<RefCell<BTreeMap<u64, HostChildStatus>>>,
    tick: Signal<u64>,
}

#[component]
pub(crate) fn ShellSurface(shell: Memo<Option<Uuid>>) -> NodeId {
    let keys = create_memo(move || shell.get().into_iter().collect::<Vec<_>>());
    view! {
        <Layers>
            <ForEach keys={keys}>|block: Uuid| {
                provide_context(Nesting::root());
                view! {
                    <TabFrame block top_bar=TopBar::Hidden />
                }
            }</ForEach>
        </Layers>
    }
}

#[component]
pub(crate) fn ShellDisplay(shell: Memo<Option<Uuid>>, display: u32) -> NodeId {
    let keys = create_memo(move || shell.get().into_iter().collect::<Vec<_>>());
    view! {
        <Layers>
            <ForEach keys={keys}>|block: Uuid| {
                provide_context(Nesting::root());
                view! {
                    <BlockRegion block region={EditorRegion::Display(display)} />
                }
            }</ForEach>
        </Layers>
    }
}

#[component]
pub(crate) fn PresentingSurface() -> NodeId {
    let editors = editors();
    let any = super::any();
    let presenting = create_memo(move || {
        any.get();
        editors.with(|open| {
            open.values()
                .find(|editor| editor.presenting_now())
                .map(PluginEditor::id)
        })
    });
    let keys = create_memo(move || presenting.get().into_iter().collect::<Vec<_>>());
    let open = create_memo(move || presenting.with(Option::is_some));
    let overlay = NodeRef::new();
    stop_on_escape(overlay);
    view! {
        <Overlay
            @node_ref=&overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            placement=Placement::Fill
            scrim=Color32::BLACK
            open={open}
            on_dismiss={move |reason: Dismiss| {
                if matches!(reason, Dismiss::Escape | Dismiss::Back) {
                    editors.with(|open| {
                        for editor in open.values_mut().filter(|editor| editor.presenting_now()) {
                            editor.stop_presenting_now();
                        }
                    });
                }
            }}
        >
            <Layers>
                <ForEach keys={keys}>|block: Uuid| {
                    provide_context(Nesting::root());
                    block_app_base::host::set_fullscreen(true);
                    on_cleanup(move || {
                        block_app_base::host::set_fullscreen(false);
                        editors.with(|open| {
                            if let Some(editor) = open.get_mut(&block) {
                                editor.stop_presenting_now();
                            }
                        });
                    });
                    view! {
                        <BlockFrame
                            block
                            chrome=Chrome::Drawn
                            content=None
                            top_bar=TopBar::Hidden
                            embedded=false
                            passive=false
                            presented=true
                        />
                    }
                }</ForEach>
            </Layers>
        </Overlay>
    }
}

#[component]
pub(crate) fn ConflictSurface() -> NodeId {
    let any = super::any();
    let comparison = create_memo(move || {
        any.get();
        crate::be::conflicts::comparing()
    });
    view! {
        <ConflictDialog comparison />
    }
}

#[component]
fn ConflictDialog(comparison: Memo<Option<crate::be::conflicts::Comparison>>) -> NodeId {
    let open = create_memo(move || comparison.with(Option::is_some));
    let error = create_memo(move || {
        comparison
            .get()
            .and_then(|comparison| comparison.error)
            .unwrap_or_default()
    });
    let failed = create_memo(move || !error.get().is_empty());
    view! {
        <Dialog
            open={open}
            title="Both versions"
            width={Size::Custom(960.0)}
            id="conflict.compare"
            actions={vec![
                DialogAction::primary("Keep merged", move || {
                    if let Some(comparison) = comparison.get_untracked() {
                        crate::be::conflicts::dismiss(comparison.block);
                    }
                }),
                DialogAction::cancel("Close"),
            ]}
            on_dismiss={crate::be::conflicts::stop_comparing}
        >
            <List spacing=Space::Lg>
                <Text style=TextStyle::CAPTION>
                    "Both devices changed this block while apart, and the block now holds both changes merged. Keep the merge, or go back to one of the versions."
                </Text>
                <List direction=Direction::Horizontal spacing=Space::Lg>
                    <ConflictSide
                        @sizing=ItemSize::Percent(50.0)
                        comparison
                        side=crate::be::conflicts::Side::Ours
                    />
                    <ConflictSide
                        @sizing=ItemSize::Percent(50.0)
                        comparison
                        side=crate::be::conflicts::Side::Theirs
                    />
                </List>
                <Show condition={failed}>
                    <Text color=TextColor::Danger>{error}</Text>
                </Show>
            </List>
        </Dialog>
    }
}

#[component]
fn ConflictSide(
    comparison: Memo<Option<crate::be::conflicts::Comparison>>,
    side: crate::be::conflicts::Side,
) -> NodeId {
    use crate::be::conflicts::Side;
    let title = match side {
        Side::Ours => "On this device",
        Side::Theirs => "From another device",
    };
    let side_id = match side {
        Side::Ours => "conflict.compare.ours",
        Side::Theirs => "conflict.compare.theirs",
    };
    let shown = create_memo(move || {
        let comparison = comparison.get()?;
        let preview = match side {
            Side::Ours => comparison.ours,
            Side::Theirs => comparison.theirs,
        };
        Some((comparison.block, preview))
    });
    let block = create_memo(move || shown.get().map(|(block, _)| block));
    let ready = create_memo(move || {
        shown
            .get()
            .is_some_and(|(_, preview)| matches!(preview, Some(Ok(_))))
    });
    view! {
        <List @test_id={side_id} spacing=Space::Md>
            <Text style=TextStyle::HEADING>{title}</Text>
            <Frame
                height={Size::Custom(360.0)}
                fill=ColorRole::Background
                border={Border::DEFAULT}
                radius=Radius::Md
            >
                <Layers>
                    <Dynamic value={shown}>|shown: Option<(Uuid, Option<Result<Uuid, String>>)>| {
                        match shown.and_then(|(_, preview)| preview) {
                            None => view! {
                                <Frame align=beui::Align::Center>
                                    <Progress width=INLINE_PROGRESS_WIDTH />
                                </Frame>
                            },
                            Some(Err(error)) => view! {
                                <Frame align=beui::Align::Center padding={Padding::all(Space::Lg)}>
                                    <Text color=TextColor::Muted>{error}</Text>
                                </Frame>
                            },
                            Some(Ok(preview)) => view! {
                                <ConflictPreview preview />
                            },
                        }
                    }</Dynamic>
                </Layers>
            </Frame>
            <List direction=Direction::Horizontal spacing=Space::Md>
                <Button
                    @test_id={format!("{side_id}.keep")}
                    label="Use this version"
                    variant=ButtonVariant::Secondary
                    disabled={create_memo(move || !ready.get())}
                    on_click={move || {
                        if let Some(block) = block.get_untracked() {
                            crate::be::conflicts::keep(block, side);
                        }
                    }}
                />
            </List>
        </List>
    }
}

#[component]
fn ConflictPreview(preview: Uuid) -> NodeId {
    let editors = editors();
    if let Some(node) = crate::be::node(preview) {
        editors.ensure(preview, node.content_type, None, None, preview);
    }
    provide_context(Nesting {
        above: store(Vec::new()),
        access: Access::View,
    });
    view! {
        <BlockFrame
            block=preview
            chrome=Chrome::None
            content=None
            top_bar=TopBar::Hidden
            embedded=true
            passive=false
        />
    }
}

fn stop_on_escape(overlay: NodeRef) {
    on_global_key(move |event: KeyEvent| {
        if event.held() {
            return true;
        }
        let Some(press) = event.press() else {
            return false;
        };
        if press.key != Key::Escape || !press.pressed || !event.target.app {
            return false;
        }
        let Some(overlay) = overlay.try_get() else {
            return false;
        };
        with_document(|document| {
            let reaches = document.escape_layer() == Some(overlay);
            if reaches {
                document.dismiss_overlay(overlay, Dismiss::Escape);
            }
            reaches
        })
    });
}

#[component]
fn BlockRegion(block: Uuid, region: EditorRegion) -> NodeId {
    let editors = editors();
    let nesting = use_context::<Nesting>().unwrap_or_else(Nesting::root);
    provide_context(nesting.within(block, nesting.access));
    let any = super::any();
    let keys = create_memo(move || {
        any.get();
        region_key(&editors, block, false)
            .into_iter()
            .collect::<Vec<_>>()
    });
    let child_view = child_view(block, region);
    view! {
        <Layers>
            <ForEach keys={keys}>|_key: RegionKey| {
                let Some(region_editor) = editors.handle(block).and_then(|handle| editors.region(&handle)) else {
                    return view! {
                        <Frame />
                    };
                };
                view! {
                    <PluginRegion
                        editor={region_editor}
                        region
                        frame={Some(FrameSpec::default())}
                        child_view
                    />
                }
            }</ForEach>
        </Layers>
    }
}

#[component]
fn TabFrame(
    block: Uuid,
    top_bar: Prop<TopBar>,
    #[prop(default = ChildReporter::default())] reporter: ChildReporter,
) -> NodeId {
    let editors = editors();
    let any = super::any();
    let stack = create_memo(move || {
        any.get();
        let mut stack = Vec::new();
        let mut next = frame_child(&editors, block);
        while let Some(id) = next {
            if id == block || stack.contains(&id) {
                break;
            }
            stack.push(id);
            next = frame_child(&editors, id);
        }
        stack
    });
    let portals = create_signal(Vec::<(Uuid, NodeId)>::new());
    let exit = ClickCallback::new(move || {
        let stack = stack.get_untracked();
        let parent = match stack.len() {
            0 | 1 => block,
            depth => stack[depth - 2],
        };
        revoke_frame_child(&editors, parent);
        block_app_base::host::request_repaint();
    });
    provide_context(TabContext {
        stack,
        top_bar,
        portals,
        exit,
    });
    let shown = create_memo(move || {
        portals.with(|held| {
            stack
                .get()
                .into_iter()
                .filter(|id| held.iter().any(|(block, _)| block == id))
                .collect::<Vec<_>>()
        })
    });
    let chrome = create_memo(move || match stack.get().is_empty() {
        true => Chrome::Drawn,
        false => Chrome::None,
    });
    view! {
        <Layers>
            <BlockFrame
                block
                chrome={chrome}
                content=None
                top_bar={top_bar}
                embedded=false
                passive=false
                reporter
            />
            <ForEach keys={shown}>|id: Uuid| {
                let node = create_memo(move || {
                    portals.with(|held| {
                        held.iter().find(|(block, _)| *block == id).map(|(_, node)| *node)
                    })
                });
                view! {
                    <Portal node={node} />
                }
            }</ForEach>
        </Layers>
    }
}

fn frame_child(editors: &Editors, block: Uuid) -> Option<Uuid> {
    let handle = editors.handle(block)?;
    plugin_host::frame_child(handle.plugin_id()?, handle.instance)
}

fn chrome_owner(editors: &Editors, block: Uuid) -> Uuid {
    let mut stack = Vec::new();
    let mut next = frame_child(editors, block);
    while let Some(id) = next {
        if id == block || stack.contains(&id) {
            break;
        }
        stack.push(id);
        next = frame_child(editors, id);
    }
    stack.last().copied().unwrap_or(block)
}

fn frame_menu(editors: &Editors, block: Uuid) -> Vec<block_plugin_api::MenuEntry> {
    let owner = chrome_owner(editors, block);
    editors
        .with(|open| open.get(&owner).map(PluginEditor::menu))
        .unwrap_or_default()
}

pub(super) fn pick_frame_menu(block: Uuid, pick: String) {
    let editors = editors();
    let owner = chrome_owner(&editors, block);
    editors.with(|open| {
        if let Some(editor) = open.get(&owner) {
            editor.pick_menu(pick);
        }
    });
}

fn revoke_frame_child(editors: &Editors, block: Uuid) {
    if let Some(handle) = editors.handle(block)
        && let Some(plugin_id) = handle.plugin_id()
    {
        plugin_host::revoke_frame_child(plugin_id, handle.instance);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Viewport {
    zoom: f32,
    pan: Vec2,
    center: Option<Pos2>,
    moved: bool,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            center: None,
            moved: false,
        }
    }
}

thread_local! {
    static VIEWPORTS: RefCell<HashMap<Uuid, Viewport>> = RefCell::new(HashMap::new());
}

#[derive(Clone, Copy)]
struct CameraOwner(Callback<Command>);

#[derive(Clone, Copy)]
enum Command {
    Pan(Vec2),
    Zoom { factor: f32, anchor: Option<Pos2> },
}

fn settle(viewport: &mut Viewport, command: Command, rect: Rect, max_zoom: f32) {
    match command {
        Command::Pan(delta) => {
            viewport.pan += delta;
            viewport.moved = true;
        }
        Command::Zoom { factor, anchor } => {
            zoom(
                viewport,
                factor,
                anchor.unwrap_or_else(|| rect.center()),
                rect,
                max_zoom,
            );
            viewport.moved = true;
        }
    }
}

fn follow(
    viewport: &mut Viewport,
    area: Option<FitArea>,
    rect: Rect,
    content: Vec2,
    max_zoom: f32,
) {
    let Some(area) = area.filter(|_| !viewport.moved) else {
        return;
    };
    let (scale, centre) = area.fit(content, rect.size());
    viewport.zoom = scale.clamp(MIN_ZOOM, max_zoom);
    viewport.pan = (content * 0.5 - centre.to_vec2()) * viewport.zoom;
}

fn zoom(viewport: &mut Viewport, factor: f32, anchor: Pos2, rect: Rect, max_zoom: f32) {
    let old = viewport.zoom;
    let new = (old * factor).clamp(MIN_ZOOM, max_zoom);
    if new != old {
        let offset = anchor - rect.center();
        viewport.pan = offset - (offset - viewport.pan) * (new / old);
        viewport.zoom = new;
    }
}

#[component]
fn BlockFrame(
    block: Uuid,
    chrome: Prop<Chrome>,
    content: Prop<Option<Rect>>,
    top_bar: Prop<TopBar>,
    embedded: bool,
    passive: Prop<bool>,
    #[prop(default = false)] presented: bool,
    #[prop(default = ChildReporter::default())] reporter: ChildReporter,
) -> NodeId {
    let any = super::any();
    let counted = store(Cell::new(false));
    let count = move || {
        if counted.with(Cell::get) {
            return;
        }
        let shown = editors().with(|open| match open.get_mut(&block) {
            Some(editor) => {
                editor.shown(true);
                true
            }
            None => false,
        });
        counted.with(|counted| counted.set(shown));
    };
    count();
    create_effect(move || {
        any.get();
        count();
    });
    on_cleanup(move || {
        if counted.with(Cell::get) {
            editors().with(|open| {
                if let Some(editor) = open.get_mut(&block) {
                    editor.shown(false);
                }
            });
        }
    });
    let shown = create_memo(move || {
        any.get();
        presented
            || editors()
                .with(|open| open.get(&block).map(|editor| !editor.presenting_now()))
                .unwrap_or(true)
    });
    let keys = create_memo(move || shown.get().then_some(block).into_iter().collect::<Vec<_>>());
    view! {
        <Layers>
            <ForEach keys={keys}>|block: Uuid|
                <BlockFrameBody
                    block
                    chrome={chrome}
                    content={content}
                    top_bar={top_bar}
                    embedded
                    passive={passive}
                    reporter={reporter}
                />
            </ForEach>
        </Layers>
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RegionKey {
    plugin: String,
    instance: EditorInstanceId,
}

fn region_key(editors: &Editors, block: Uuid, preview: bool) -> Option<RegionKey> {
    let handle = editors.handle(block)?;
    if preview && !handle.preview {
        return None;
    }
    Some(RegionKey {
        plugin: handle.plugin_id()?.to_owned(),
        instance: handle.instance,
    })
}

#[derive(Clone, PartialEq)]
enum Resolution {
    NoAccess,
    Missing,
    Locked,
    Unsupported(Uuid),
    Stale(Uuid, crate::be::Stale),
    Ready { region: RegionKey, access: Access },
}

#[component]
fn ProgramSurface(block: Uuid) -> NodeId {
    let any = super::any();
    let shown = create_memo(move || {
        any.get();
        crate::programs::shown(block)
    });
    view! {
        <Layers>
            <Dynamic value={shown}>|shown: Option<crate::programs::Shown>| {
                match shown {
                    Some(crate::programs::Shown::Window(window)) => view! {
                        <WindowSurface window={window} />
                    },
                    Some(crate::programs::Shown::Closed(name)) => view! {
                        <ProgramClosed name block />
                    },
                    Some(crate::programs::Shown::Failed(problem)) => view! {
                        <Notice
                            item={HostItem::Notice {
                                text: problem,
                                busy: false,
                                icon: Some(beui::icons::ICON_APP_WINDOW),
                            }}
                        />
                    },
                    Some(crate::programs::Shown::Opening(name)) => view! {
                        <Notice
                            item={HostItem::Notice {
                                text: format!("Opening in {name}…"),
                                busy: true,
                                icon: Some(beui::icons::ICON_APP_WINDOW),
                            }}
                        />
                    },
                    None => view! {
                        <Frame />
                    },
                }
            }</Dynamic>
        </Layers>
    }
}

#[component]
fn ProgramClosed(name: String, block: Uuid) -> NodeId {
    let content = create_memo(move || HostItem::Notice {
        text: format!("{name} is closed."),
        busy: false,
        icon: Some(beui::icons::ICON_APP_WINDOW),
    });
    view! {
        <Frame align=beui::Align::Center>
            <List spacing=Space::Sm align=Align::Center>
                <HostItemFace content />
                <Button
                    label="Open again"
                    variant=ButtonVariant::Primary
                    @test_id={"program.reopen"}
                    on_click={move || crate::programs::reopen(block)}
                />
            </List>
        </Frame>
    }
}

#[component]
fn Notice(item: HostItem) -> NodeId {
    let content = create_memo(move || item.clone());
    view! {
        <Frame align=beui::Align::Center>
            <HostItemFace content />
        </Frame>
    }
}

#[component]
fn BlockFrameBody(
    block: Uuid,
    chrome: Prop<Chrome>,
    content: Prop<Option<Rect>>,
    top_bar: Prop<TopBar>,
    embedded: bool,
    passive: Prop<bool>,
    reporter: ChildReporter,
) -> NodeId {
    let editors = editors();
    let nesting = use_context::<Nesting>().unwrap_or_else(Nesting::root);
    let any = super::any();
    let resolution = create_memo(move || {
        any.get();
        let access = editors.access(block, nesting.access);
        if !access.can_view() {
            return Resolution::NoAccess;
        }
        let Some(handle) = editors.handle(block) else {
            return Resolution::Missing;
        };
        if crate::be::locked(block) {
            return Resolution::Locked;
        }
        if let Some(stale) = crate::be::staleness(block) {
            return Resolution::Stale(handle.block_type, stale);
        }
        match region_key(&editors, block, false) {
            Some(region) => Resolution::Ready { region, access },
            None => Resolution::Unsupported(handle.block_type),
        }
    });
    let conflicted = create_memo(move || {
        any.get();
        match resolution.get() {
            Resolution::Ready { .. } => crate::be::conflicts::conflicted(block)
                .map(|conflicted| conflicted.count)
                .into_iter()
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        }
    });
    view! {
        <Layers>
            <Dynamic value={resolution}>|resolution: Resolution| {
                match resolution {
                    Resolution::NoAccess => {
                        let item = HostItem::Notice {
                            text: "No access".to_owned(),
                            busy: false,
                            icon: Some(beui::icons::ICON_LOCK),
                        };
                        view! {
                            <Notice item />
                        }
                    }
                    Resolution::Missing => view! {
                        <Frame />
                    },
                    Resolution::Locked => {
                        let item = HostItem::Notice {
                            text: "Locked: its key has not been shared with you yet"
                                .to_owned(),
                            busy: false,
                            icon: Some(beui::icons::ICON_LOCK),
                        };
                        view! {
                            <Notice item />
                        }
                    }
                    Resolution::Unsupported(block_type) => {
                        let item = HostItem::Unsupported { block, block_type };
                        view! {
                            <Notice item />
                        }
                    }
                    Resolution::Stale(block_type, stale) => view! {
                        <StaleBlock block block_type stale />
                    },
                    Resolution::Ready { access, .. } => {
                        let editors = self::editors();
                        let Some(handle) = editors.handle(block) else {
                            return view! {
                                <Frame />
                            };
                        };
                        let Some(region_editor) = editors.region(&handle) else {
                            return view! {
                                <Frame />
                            };
                        };
                        view! {
                            <BlockFrameView
                                block
                                access
                                handle
                                region_editor
                                chrome={chrome}
                                content={content}
                                top_bar={top_bar}
                                embedded
                                passive={passive}
                                reporter={reporter}
                            />
                        }
                    }
                }
            }</Dynamic>
            <ForEach keys={conflicted}>|count: usize|
                <ConflictNotice block count />
            </ForEach>
        </Layers>
    }
}

#[component]
fn BlockFrameView(
    block: Uuid,
    access: Access,
    handle: Handle,
    region_editor: RegionEditor,
    chrome: Prop<Chrome>,
    content: Prop<Option<Rect>>,
    top_bar: Prop<TopBar>,
    embedded: bool,
    passive: Prop<bool>,
    reporter: ChildReporter,
) -> NodeId {
    let ChildReporter {
        bar: on_bar_action,
        focus: on_focus,
    } = reporter;
    provide_context(FocusedWithin(on_focus));
    let nesting = use_context::<Nesting>().unwrap_or_else(Nesting::root);
    provide_context(nesting.within(block, access));
    let plugin_id = store(region_editor.plugin_id().to_owned());
    let instance = region_editor.instance;
    let framed = create_memo(move || content.get().is_some());
    create_effect(move || {
        let _ = framed.get();
        plugin_host::hold(&plugin_id.value(), instance, EditorRegion::Frame);
    });
    let revision = super::listen(&plugin_id.value());
    let rect_held = create_signal(Rect::ZERO);
    let rect = rect_held.read_only();
    let read_only = !access.can_edit();
    let placed = embedded || content.get_untracked().is_some();
    let pan_and_zoom = handle.pan_and_zoom && !placed;
    let max_zoom = handle.max_zoom.map_or(MAX_ZOOM, |zoom| zoom as f32);
    let viewport = create_signal(
        VIEWPORTS
            .with(|viewports| viewports.borrow().get(&block).copied())
            .unwrap_or_default(),
    );
    create_effect(move || {
        let state = viewport.get();
        VIEWPORTS.with(|viewports| viewports.borrow_mut().insert(block, state));
    });
    let viewport_rect = create_memo(move || {
        revision.get();
        let rect = rect.get();
        plugin_host::frame_rects(&plugin_id.value(), instance)
            .map(|rects| rects.content.translate(rect.min.to_vec2()))
            .filter(Rect::is_positive)
            .unwrap_or(rect)
    });
    let content_size = create_memo(move || {
        revision.get();
        let size = viewport_rect.get().size().max(Vec2::splat(1.0));
        let intrinsic =
            plugin_host::intrinsic_size(&plugin_id.value(), instance).unwrap_or(vec2(420.0, 240.0));
        vec2(size.x.max(intrinsic.x), size.y.max(intrinsic.y))
    });
    let content_rect = create_memo(move || {
        let state = viewport.get();
        Rect::from_center_size(
            viewport_rect.get().center() + state.pan,
            content_size.get() * state.zoom,
        )
    });
    let editor_view = create_memo(move || {
        if !handle.pan_and_zoom {
            return None;
        }
        let origin = rect.get().min;
        if !placed {
            let state = viewport.get();
            return Some(EditorView {
                rect: content_rect.get().translate(-origin.to_vec2()),
                scale: state.zoom,
            });
        }
        revision.get();
        let shown = content.get().unwrap_or(rect.get());
        let scale = plugin_host::intrinsic_size(&plugin_id.value(), instance)
            .filter(|intrinsic| intrinsic.x > 0.0 && intrinsic.y > 0.0)
            .map_or(1.0, |intrinsic| {
                (shown.width() / intrinsic.x).min(shown.height() / intrinsic.y)
            });
        Some(EditorView {
            rect: shown.translate(-origin.to_vec2()),
            scale,
        })
    });
    let fitted = create_signal(None::<FitArea>);
    let fit = move || {
        if !pan_and_zoom {
            return;
        }
        let mut state = viewport.get_untracked();
        state.moved = false;
        follow(
            &mut state,
            fitted.get_untracked(),
            viewport_rect.get_untracked(),
            content_size.get_untracked(),
            max_zoom,
        );
        if state != viewport.get_untracked() {
            viewport.set(state);
        }
    };
    let owner = use_context::<CameraOwner>();
    let command = move |command: Command| {
        if !pan_and_zoom {
            if let (Some(owner), true) = (owner, handle.pan_and_zoom) {
                owner.0.call(command);
            }
            return;
        }
        let mut state = viewport.get_untracked();
        settle(&mut state, command, viewport_rect.get_untracked(), max_zoom);
        if state != viewport.get_untracked() {
            viewport.set(state);
        }
    };
    if pan_and_zoom {
        provide_context(CameraOwner(Callback::new(command)));
    }
    create_effect(move || {
        if !pan_and_zoom {
            return;
        }
        let rect = viewport_rect.get();
        let content = content_size.get();
        let area = fitted.get().filter(|_| chrome.get() == Chrome::Drawn);
        let mut state = viewport.get_untracked();
        if let Some(previous) = state.center.replace(rect.center()) {
            state.pan += previous - rect.center();
        }
        follow(&mut state, area, rect, content, max_zoom);
        if state != viewport.get_untracked() {
            viewport.set(state);
        }
    });
    let pan_drags = create_signal(0u64);
    create_effect(move || {
        revision.get();
        let rect = rect.get_untracked();
        for change in plugin_host::take_view_changes(&plugin_id.value(), instance) {
            command(match change {
                ViewChange::Pan(by) => Command::Pan(by),
                ViewChange::Zoom { factor, anchor } => Command::Zoom {
                    factor,
                    anchor: anchor.map(|at| rect.min + at.to_vec2()),
                },
                ViewChange::KeepFitted(area) => {
                    fitted.set(area);
                    continue;
                }
                ViewChange::Fit => {
                    fit();
                    continue;
                }
                ViewChange::PanDrag => {
                    pan_drags.update(|asked| *asked += 1);
                    continue;
                }
            });
        }
        let actions = plugin_host::take_bar_actions(&plugin_id.value(), instance);
        for action in actions {
            on_bar_action.call(action);
        }
    });
    if let Some(tab) = use_context::<TabContext>() {
        create_effect(move || {
            revision.get();
            if plugin_host::take_leaving(&plugin_id.value(), instance) {
                tab.exit.call();
            }
        });
    }
    let frame = create_memo(move || {
        let origin = rect.get().min;
        Some(FrameSpec {
            chrome: match chrome.get() {
                Chrome::Drawn => FrameChrome::Drawn,
                Chrome::None => FrameChrome::None,
            },
            content: content
                .get()
                .map(|content| content.translate(-origin.to_vec2())),
            top_bar: top_bar.get(),
            dialog: None,
        })
    });
    let gestures =
        handle.pan != PanMode::None && (pan_and_zoom || owner.is_some() && handle.pan_and_zoom);
    let pan_drag = (gestures && handle.pan == PanMode::OneFinger).then(|| PanDrag {
        asked: pan_drags.read_only(),
        pan: Callback::new(move |delta: Vec2| command(Command::Pan(delta))),
    });
    let steered =
        create_memo(move || (!read_only && !handle.pan_and_zoom).then(|| content_rect.get()));
    let scroll = move |gesture: ScrollGesture| {
        if !gestures
            || steered
                .get_untracked()
                .is_some_and(|rect| rect.contains(gesture.pos))
        {
            return;
        }
        match gesture.modifiers.ctrl && gesture.delta.y != 0.0 {
            true => command(Command::Zoom {
                factor: (gesture.delta.y * 0.002).exp(),
                anchor: Some(gesture.pos),
            }),
            false => command(Command::Pan(gesture.delta)),
        }
    };
    let zoom = move |gesture: ZoomGesture| {
        if gestures {
            command(Command::Zoom {
                factor: gesture.factor,
                anchor: Some(gesture.pos),
            });
        }
    };
    let pan = move |delta: Vec2| {
        if gestures {
            command(Command::Pan(delta));
        }
    };
    let gesture_area = create_memo(move || {
        if !gestures {
            return beui::Area::Nowhere;
        }
        revision.get();
        let origin = rect.get().min.to_vec2();
        let mut area = beui::HitRegion::rect(viewport_rect.get().translate(-origin));
        if let Some(steered) = steered.get() {
            area = area.without(steered.translate(-origin));
        }
        if let Some(frame) = plugin_host::frame_rects(&plugin_id.value(), instance) {
            for floating in frame.floating {
                area = area.without(floating);
            }
        }
        beui::Area::Within(area)
    });
    let recognised = beui::unstyled::gestures::pan_zoom(pan, zoom);
    let placed_zoom = create_memo(move || {
        if handle.pan_and_zoom || !embedded || content.get().is_some() {
            return 1.0;
        }
        revision.get();
        let shown = rect.get();
        plugin_host::assigned_size(&plugin_id.value(), instance)
            .filter(|assigned| assigned.x > 0.0 && assigned.y > 0.0)
            .map_or(1.0, |assigned| {
                (shown.width() / assigned.x).min(shown.height() / assigned.y)
            })
    });
    let child_view = child_view(block, EditorRegion::Frame);
    let built = view! {
        <Interactive
            on_wheel={beui::unstyled::gestures::first_of([
                recognised.wheel,
                beui::unstyled::gestures::scroll_wheel(None, scroll),
            ])}
            on_pointer={recognised.pointer}
            gesture_area={gesture_area}
        >
            <PluginRegion
                editor={region_editor}
                region=EditorRegion::Frame
                frame={frame}
                view={editor_view}
                zoom={placed_zoom}
                passive={passive}
                pan_drag
                on_focus
                child_view
            />
        </Interactive>
    };
    let rect_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = rect_measured.get() {
            rect_held.set(measured.rect);
        }
    });
    built
}

pub(super) fn child_view(parent: Uuid, region: EditorRegion) -> ChildView {
    store(Box::new(
        move |key: ChildId, child: Memo<Option<HostChild>>, rect: Memo<Rect>| {
            view! {
                <HostedChild parent region key child rect />
            }
        },
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    Missing,
    Unavailable,
    Preview,
    OwnFrame,
    FrameChild,
    Embedded,
    Panel(HostPanel),
    Window(HostWindowId),
    Program(Uuid),
    Creation,
    ArtifactSettings(Uuid),
    DockTab(Uuid, u64),
    Picker,
    NoPicker,
}

pub(crate) fn hosted_parent(instance: EditorInstanceId) -> Uuid {
    Uuid::from_u64_pair(0x686f_7374_6564_0000, instance.0)
}

#[component]
fn HostedChild(
    parent: Uuid,
    region: EditorRegion,
    key: ChildId,
    child: Memo<Option<HostChild>>,
    rect: Memo<Rect>,
) -> NodeId {
    let editors = editors();
    let nesting = use_context::<Nesting>().unwrap_or_else(Nesting::root);
    let tab = use_context::<TabContext>();
    let any = super::any();
    let inherited = super::region::occlusion();
    provide_context(super::region::Occlusion(create_memo(move || {
        let mut rects = inherited
            .as_ref()
            .map(|inherited| inherited.get())
            .unwrap_or_default();
        rects.extend(child.with(|child| {
            child
                .as_ref()
                .map(|child| child.occluders.clone())
                .unwrap_or_default()
        }));
        rects
    })));
    let content = create_memo(move || child.get().map(|child| child.content));
    let creation_key: CreationKey = (parent, region, key);
    on_cleanup(move || {
        editors.forget_creation(&creation_key);
        editors.forget_picker(&creation_key);
        editors.hide_settings(&creation_key);
    });
    let kind = create_memo(move || {
        any.get();
        let (id, block_type, view_block, editor) = match content.get() {
            None => return Kind::Missing,
            Some(HostContent::Panel(panel)) => return Kind::Panel(panel),
            Some(HostContent::Window(window)) => {
                return match super::shell().get() == Some(parent) {
                    true => Kind::Window(window),
                    false => Kind::Unavailable,
                };
            }
            Some(HostContent::DockTab { block, tab }) => {
                let shown = child.with(|child| {
                    child
                        .as_ref()
                        .is_some_and(|child| child.mode != ChildMode::Preview)
                });
                return match shown && editors.handle(block).is_some() {
                    true => Kind::DockTab(block, tab),
                    false => Kind::Unavailable,
                };
            }
            Some(HostContent::ArtifactSettings(block)) => {
                editors.show_settings(creation_key, block);
                return Kind::ArtifactSettings(block);
            }
            Some(HostContent::Creation { editor, template }) => {
                return match editors.ensure_creation(creation_key, editor, &template) {
                    true => Kind::Creation,
                    false => Kind::Unavailable,
                };
            }
            Some(HostContent::Picker {
                filter,
                parent,
                touch,
            }) => {
                let request = crate::plugin_host::PickerRequest {
                    filter,
                    parent,
                    touch,
                };
                return match editors.ensure_picker(creation_key, request) {
                    true => Kind::Picker,
                    false => Kind::NoPicker,
                };
            }
            Some(HostContent::Block {
                block_id,
                block_type,
                view_block,
                editor,
            }) => (block_id, block_type, view_block, editor),
        };
        if crate::be::revisions::released(id) {
            return Kind::Unavailable;
        }
        if nesting.above.with(|above| above.contains(&id)) {
            return Kind::Unavailable;
        }
        if let Some(program) = editor.filter(|editor| crate::programs::opens(*editor)) {
            crate::programs::show(id, block_type, program, parent);
            return Kind::Program(id);
        }
        editors.ensure(id, block_type, view_block, editor, parent);
        let Some(child) = child.get() else {
            return Kind::Missing;
        };
        let in_stack = tab
            .as_ref()
            .is_some_and(|tab| tab.stack.get().contains(&id));
        match child.mode {
            ChildMode::Preview => Kind::Preview,
            _ if child.frame_owner && child.own_frame => Kind::OwnFrame,
            _ if child.frame_owner && in_stack => Kind::FrameChild,
            _ => Kind::Embedded,
        }
    });
    create_effect(move || {
        let Some(child) = child.get() else {
            return;
        };
        if let (Some(size), Some(handle)) = (
            child.intrinsic,
            child.block_id().and_then(|id| editors.handle(id)),
        ) && let Some(plugin_id) = handle.plugin_id()
        {
            plugin_host::resized(plugin_id, handle.instance, size);
        }
    });
    if let Some(reports) = use_context::<ChildReports>() {
        let status = create_memo(move || {
            any.get();
            let child = child.get()?;
            let available = !matches!(
                kind.get(),
                Kind::Unavailable | Kind::Missing | Kind::NoPicker
            );
            let mut status = status_of(&editors, &child, available);
            if let (Kind::Preview, Some(block)) = (kind.get(), child.block_id()) {
                status.preview = preview_of(&editors, block);
            }
            if let Some((progress, height, dialog)) = editors.creation_status(&creation_key) {
                status.creation = progress;
                status.intrinsic = height.map(|height| vec2(0.0, height));
                status.dialog = dialog;
            }
            if kind.get() == Kind::NoPicker {
                status.error = Some(NO_PICKER.to_owned());
            }
            if let Some((picked, intrinsic, card)) = editors.picker_status(&creation_key) {
                status.picked = picked;
                status.card = card;
                status.intrinsic = intrinsic;
            }
            if let Kind::ArtifactSettings(block) = kind.get()
                && let Some((height, dialog)) = editors.settings_status(block)
            {
                status.intrinsic = Some(vec2(0.0, height));
                status.dialog = dialog;
            }
            Some(status)
        });
        create_effect(move || {
            let status = status.get();
            reports.statuses.with(|statuses| {
                let mut statuses = statuses.borrow_mut();
                match status {
                    Some(status) => statuses.insert(key.0, status),
                    None => statuses.remove(&key.0),
                };
            });
            reports.tick.update(|tick| *tick += 1);
        });
        on_cleanup(move || {
            reports
                .statuses
                .with(|statuses| statuses.borrow_mut().remove(&key.0));
            reports.tick.update(|tick| *tick += 1);
        });
    }
    let local = create_memo(move || {
        child.get().map_or(Rect::ZERO, |child| {
            child.rect.translate(-child.clip.min.to_vec2())
        })
    });
    let x = create_memo(move || local.get().min.x);
    let y = create_memo(move || local.get().min.y);
    let width = create_memo(move || local.get().width());
    let height = create_memo(move || local.get().height());
    let keys = create_memo(move || {
        content
            .get()
            .map(|content| {
                (
                    content.block_id().unwrap_or_default(),
                    kind.get(),
                    content.editor(),
                )
            })
            .into_iter()
            .collect::<Vec<_>>()
    });
    let _ = rect;
    let within = use_context::<FocusedWithin>().map_or_else(Callback::default, |within| within.0);
    view! {
        <Canvas>
            <CanvasItem x={x} y={y} width={width} height={height}>
                <Layers>
                    <ForEach keys={keys}>|(id, kind, _): (Uuid, Kind, Option<Uuid>)| {
                        let reporter = reporter_for(parent, region, key, within);
                        match kind {
                            Kind::Missing | Kind::Unavailable | Kind::NoPicker => view! {
                                <Frame />
                            },
                            Kind::Picker => match editors.picker_region(&creation_key) {
                                Some(hosted) => {
                                    let dialog = create_memo(move || child.get().and_then(|child| child.dialog));
                                    view! {
                                        <CreationSurface hosted dialog />
                                    }
                                }
                                None => view! {
                                    <Frame />
                                },
                            },
                            Kind::Panel(panel) => view! {
                                <HostPanelSurface panel={panel} />
                            },
                            Kind::Window(window) => view! {
                                <WindowSurface window={window} />
                            },
                            Kind::Program(block) => view! {
                                <ProgramSurface block />
                            },
                            Kind::DockTab(block, tab) => view! {
                                <DockTabSurface block tab reporter />
                            },
                            Kind::ArtifactSettings(_) => {
                                let dialog = create_memo(move || child.get().and_then(|child| child.dialog));
                                view! {
                                    <HostedSurface id=SurfaceId::ArtifactSettings dialog />
                                }
                            }
                            Kind::Creation => match editors.creation_region(&creation_key) {
                                Some(hosted) => {
                                    let dialog = create_memo(move || child.get().and_then(|child| child.dialog));
                                    view! {
                                        <CreationSurface hosted dialog />
                                    }
                                }
                                None => view! {
                                    <Frame />
                                },
                            },
                            Kind::Preview => view! {
                                <BlockPreview block=id />
                            },
                            Kind::OwnFrame => {
                                let top_bar = create_memo(move || {
                                    child.get().map_or(TopBar::Hidden, |child| child.top_bar)
                                });
                                view! {
                                    <TabFrame block=id top_bar={top_bar} reporter />
                                }
                            }
                            Kind::FrameChild => {
                                let Some(tab) = use_context::<TabContext>() else {
                                    return view! {
                                        <Frame />
                                    };
                                };
                                let content = create_memo(move || {
                                    child.get().map(|child| child.view_rect.intersect(child.clip))
                                });
                                let chrome = create_memo(move || {
                                    match tab.stack.get().last() == Some(&id) {
                                        true => Chrome::Drawn,
                                        false => Chrome::None,
                                    }
                                });
                                let top_bar = tab.top_bar;
                                let node = view! {
                                    <BlockFrame
                                        block=id
                                        chrome={chrome}
                                        content={content}
                                        top_bar={top_bar}
                                        embedded=false
                                        passive=false
                                        reporter
                                    />
                                };
                                tab.portals.update(|held| {
                                    held.retain(|(block, _)| *block != id);
                                    held.push((id, node));
                                });
                                on_cleanup(move || {
                                    tab.portals.update(|held| {
                                        held.retain(|entry| *entry != (id, node));
                                    });
                                });
                                view! {
                                    <Frame />
                                }
                            }
                            Kind::Embedded => {
                                let passive = create_memo(move || {
                                    child.get().is_none_or(|child| child.mode == ChildMode::Passive)
                                });
                                view! {
                                    <BlockFrame
                                        block=id
                                        chrome=Chrome::None
                                        content=None
                                        top_bar=TopBar::Hidden
                                        embedded=true
                                        passive={passive}
                                        reporter
                                    />
                                }
                            }
                        }
                    }</ForEach>
                </Layers>
            </CanvasItem>
        </Canvas>
    }
}

#[component]
fn DockTabSurface(block: Uuid, tab: u64, reporter: ChildReporter) -> NodeId {
    let editors = editors();
    let nesting = use_context::<Nesting>().unwrap_or_else(Nesting::root);
    let Some(region_editor) = editors
        .handle(block)
        .and_then(|handle| editors.region(&handle))
    else {
        return view! {
            <Frame />
        };
    };
    provide_context(FocusedWithin(reporter.focus));
    provide_context(nesting.within(block, editors.access(block, nesting.access)));
    let region = EditorRegion::Tab(tab);
    let child_view = child_view(block, region);
    view! {
        <PluginRegion editor={region_editor} region child_view on_focus={reporter.focus} />
    }
}

#[component]
fn CreationSurface(hosted: HostedRegion, dialog: Memo<Option<DialogFrame>>) -> NodeId {
    let child_view = child_view(hosted_parent(hosted.editor.instance), hosted.region);
    let frame = hosted.frame;
    view! {
        <PluginRegion
            editor={hosted.editor}
            region={hosted.region}
            frame={Prop::dynamic(move || {
                frame.map(|frame| FrameSpec {
                    dialog: dialog.get(),
                    ..frame
                })
            })}
            child_view
        />
    }
}

fn status_of(editors: &Editors, child: &HostChild, available: bool) -> HostChildStatus {
    let block = child.block_id().unwrap_or_default();
    let (interaction, capabilities, resize, intrinsic, aspect_ratio) =
        editors.with(|open| match open.get_mut(&block) {
            Some(editor) if available => (
                match editor.direct_editor_interaction() {
                    crate::editors::DirectEditorInteraction::Live => {
                        block_plugin_api::InteractionMode::Live
                    }
                    crate::editors::DirectEditorInteraction::Playback => {
                        block_plugin_api::InteractionMode::Playback
                    }
                    crate::editors::DirectEditorInteraction::Preview => {
                        block_plugin_api::InteractionMode::Preview
                    }
                },
                {
                    let capabilities = editor.direct_editor_capabilities();
                    block_plugin_api::EditorCapabilities {
                        rotation: capabilities.allow_rotation,
                        preserve_aspect_ratio: capabilities.preserve_aspect_ratio,
                        pan_and_zoom: capabilities.supports_pan_and_zoom,
                        max_zoom: capabilities.max_zoom,
                        pan: capabilities.pan,
                    }
                },
                match editor.direct_editor_resize() {
                    crate::editors::DirectEditorResize::Horizontal => {
                        block_plugin_api::ResizeMode::Horizontal
                    }
                    crate::editors::DirectEditorResize::Vertical => {
                        block_plugin_api::ResizeMode::Vertical
                    }
                    crate::editors::DirectEditorResize::Both => block_plugin_api::ResizeMode::Both,
                    crate::editors::DirectEditorResize::None => block_plugin_api::ResizeMode::None,
                },
                editor.direct_editor_intrinsic_size(),
                editor.render_aspect_ratio(),
            ),
            _ => (
                block_plugin_api::InteractionMode::Preview,
                block_plugin_api::EditorCapabilities::default(),
                block_plugin_api::ResizeMode::None,
                None,
                None,
            ),
        });
    HostChildStatus {
        child: child.child,
        available,
        intrinsic,
        aspect_ratio,
        active: available
            && match child.mode {
                ChildMode::Active => child.frame_owner,
                _ => child.is_active(),
            },
        interaction,
        capabilities,
        resize,
        error: crate::be::revisions::error(block)
            .or_else(|| (!available).then(|| CHILD_UNAVAILABLE.to_owned())),
        creation: None,
        preview: ChildPreview::Hosted,
        dialog: DialogStatus::default(),
        picked: None,
        card: false,
        menu: match available && child.frame_owner {
            true => frame_menu(editors, block),
            false => Vec::new(),
        },
    }
}

fn reporter_for(
    parent: Uuid,
    region: EditorRegion,
    key: ChildId,
    within: Callback<()>,
) -> ChildReporter {
    let report_to = editors().handle(parent);
    let on_bar_action = report_to.clone().and_then(|parent| {
        let plugin_id = parent.plugin_id()?.to_owned();
        Some(Callback::new(move |action: BarAction| {
            plugin_host::report_child_bars(
                &plugin_id,
                parent.instance,
                region,
                vec![(key, action)],
            );
        }))
    });
    let plugin_id = report_to
        .as_ref()
        .and_then(|parent| parent.plugin_id().map(str::to_owned));
    let instance = report_to.map(|parent| parent.instance);
    let on_focus = Callback::new(move |()| {
        if let (Some(plugin_id), Some(instance)) = (&plugin_id, instance) {
            plugin_host::report_child_focus(plugin_id, instance, region, key);
        }
        within.call(());
    });
    ChildReporter {
        bar: on_bar_action.unwrap_or_default(),
        focus: on_focus,
    }
}

#[component]
fn BlockPreview(block: Uuid) -> NodeId {
    let editors = editors();
    let any = super::any();
    let keys = create_memo(move || {
        any.get();
        region_key(&editors, block, true)
            .into_iter()
            .collect::<Vec<_>>()
    });
    let child_view = child_view(block, EditorRegion::Preview);
    view! {
        <Layers>
            <ForEach keys={keys}>|_key: RegionKey| {
                let Some(region_editor) = editors.handle(block).and_then(|handle| editors.region(&handle)) else {
                    return view! {
                        <Frame />
                    };
                };
                view! {
                    <PluginRegion
                        editor={region_editor}
                        region=EditorRegion::Preview
                        offscreen=true
                        child_view
                    />
                }
            }</ForEach>
        </Layers>
    }
}

fn preview_of(editors: &Editors, block: Uuid) -> ChildPreview {
    let Some(handle) = editors.handle(block).filter(|handle| handle.preview) else {
        return ChildPreview::Label;
    };
    let Some(plugin) = handle.plugin_id() else {
        return ChildPreview::Label;
    };
    plugin_host::preview_image(plugin, handle.instance)
        .map_or(ChildPreview::Waiting, ChildPreview::Image)
}

pub(super) fn reports() -> (ChildReports, ReadSignal<u64>) {
    let tick = create_signal(0u64);
    (
        ChildReports {
            statuses: store(RefCell::new(BTreeMap::new())),
            tick,
        },
        tick.read_only(),
    )
}

pub(super) fn provide_reports(reports: ChildReports) {
    provide_context(reports);
}

pub(super) fn statuses(reports: &ChildReports) -> Vec<HostChildStatus> {
    reports
        .statuses
        .with(|statuses| statuses.borrow().values().cloned().collect())
}

#[cfg(test)]
mod tests;
